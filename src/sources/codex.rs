use super::{add_message, is_subagent, new_session, string, text, timestamp};
use crate::model::{Conversation, Message, digest};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub(super) fn parse_codex(records: &[Value], path: &Path) -> Result<Vec<Conversation>> {
    let mut sessions = BTreeMap::<String, Conversation>::new();
    let mut current = None;
    let mut subagent = false;
    let mut occurrences = BTreeMap::<(String, String), usize>::new();
    for record in records {
        let kind = record.get("type").and_then(Value::as_str).unwrap_or("");
        let payload = record.get("payload");
        if kind == "session_meta" {
            let payload = payload.context("Codex session metadata has no payload")?;
            let id = string(payload, "id")
                .or_else(|| string(payload, "session_id"))
                .context("Codex session metadata has no stable ID")?;
            // Forked rollouts start with the child's identity, followed by
            // inherited parent metadata. Never switch identities or downgrade
            // a subagent's requests to first-person user statements.
            if current.is_some() {
                continue;
            }
            subagent =
                is_subagent(payload.get("source")) || is_subagent(payload.get("thread_source"));
            let session = sessions.entry(id.clone()).or_insert_with(|| {
                new_session("codex", id.clone(), path, "codex-rollout-jsonl-v1")
            });
            session.created_at = timestamp(payload.get("timestamp"))?;
            // Keep source identity and project/version metadata, without
            // treating provider instructions or configuration as user speech.
            for key in [
                "cwd",
                "cli_version",
                "originator",
                "source",
                "thread_source",
            ] {
                if let Some(value) = payload.get(key) {
                    session.metadata[key] = value.clone();
                }
            }
            current = Some(id);
            continue;
        }
        if kind != "response_item" {
            continue;
        }
        let payload = payload.context("Codex response item has no payload")?;
        if payload.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let id = current
            .as_ref()
            .context("Codex message appears before session identity")?;
        let mut role = string(payload, "role").unwrap_or_else(|| "unknown".into());
        if subagent && role == "user" {
            role = "agent".into();
        }
        let content = text(payload.get("content"));
        if role == "user" && content.trim_start().starts_with("<codex_internal_context") {
            role = "system".into();
        }
        let source_id = if let Some(value) = string(payload, "id") {
            value
        } else if let Some(ordinal) = record.get("ordinal").and_then(Value::as_u64) {
            format!("ordinal-{ordinal}")
        } else {
            let fingerprint = digest(&serde_json::to_vec(record)?);
            let occurrence = occurrences
                .entry((id.clone(), fingerprint.clone()))
                .or_default();
            *occurrence += 1;
            format!("record-{fingerprint}-{}", *occurrence)
        };
        add_message(
            sessions.get_mut(id).expect("current session exists"),
            Message {
                source_id,
                parent_source_id: None,
                role,
                content,
                created_at: timestamp(record.get("timestamp"))?,
                metadata: record.clone(),
            },
        )?;
    }
    Ok(sessions.into_values().collect())
}
