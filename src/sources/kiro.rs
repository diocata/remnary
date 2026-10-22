use super::{new_session, string};
use crate::model::{Conversation, Message, digest};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub(super) fn parse_kiro(record: &Value, path: &Path) -> Result<Vec<Conversation>> {
    let id = string(record, "executionId").context("Kiro chat is missing executionId")?;
    let messages = record
        .get("chat")
        .and_then(Value::as_array)
        .context("Kiro chat must have a chat array")?;
    let mut conversation = new_session("kiro", id, path, "kiro-ide-chat-v1");
    let mut metadata = record
        .as_object()
        .context("Kiro chat must be an object")?
        .clone();
    metadata.remove("chat");
    conversation.metadata["source_metadata"] = Value::Object(metadata);
    // The observed Kiro IDE format records session times in JavaScript
    // milliseconds. It provides no per-message time; those remain unknown.
    for (key, target) in [
        ("startTime", &mut conversation.created_at),
        ("endTime", &mut conversation.updated_at),
    ] {
        let value = record.get("metadata").and_then(|m| m.get(key));
        if let Some(Value::Number(number)) = value {
            *target = Some(number.as_f64().context("invalid Kiro session timestamp")? / 1000.0);
        }
    }
    let mut occurrences = BTreeMap::<String, usize>::new();
    let mut parent = None;
    for message in messages {
        ensure!(message.is_object(), "Kiro chat entry must be an object");
        let original_role = string(message, "role").unwrap_or_else(|| "unknown".into());
        let mut role = match original_role.as_str() {
            "human" => "user",
            "bot" => "assistant",
            other => other,
        }
        .to_owned();
        let mut content = message
            .get("content")
            .and_then(Value::as_str)
            .context("Kiro chat content must be text")?
            .to_owned();
        if original_role == "human" {
            if content
                .trim_start()
                .starts_with("<identity>\nYou are Kiro,")
            {
                // The IDE serializes its own system prompt as `human`.
                // Preserve it for provenance, without attributing it to the user.
                role = "system".into();
            } else if let Some((request, _environment)) =
                content.split_once("\n<EnvironmentContext>")
            {
                // This known IDE suffix is provider context. The complete raw
                // entry remains in metadata; normalized text is actual speech.
                content = request.trim_end().to_owned();
            }
        }
        let fingerprint = digest(&serde_json::to_vec(message)?);
        let occurrence = occurrences.entry(fingerprint.clone()).or_default();
        *occurrence += 1;
        let source_id = format!("message-{fingerprint}-{}", *occurrence);
        conversation.messages.push(Message {
            source_id: source_id.clone(),
            parent_source_id: parent,
            role,
            content,
            created_at: None,
            metadata: message.clone(),
        });
        parent = Some(source_id);
    }
    Ok(vec![conversation])
}
