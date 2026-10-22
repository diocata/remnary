use super::{add_message, new_session, string, text, timestamp};
use crate::model::{Conversation, Message};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub(super) fn parse_claude(records: &[Value], path: &Path) -> Result<Vec<Conversation>> {
    let mut sessions = BTreeMap::<String, Conversation>::new();
    let sidecar = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| s.starts_with("agent-"));
    for record in records {
        let kind = record.get("type").and_then(Value::as_str).unwrap_or("");
        if !matches!(kind, "user" | "assistant") {
            continue;
        }
        let mut id =
            string(record, "sessionId").context("Claude Code message is missing sessionId")?;
        if let Some(sidecar) = sidecar {
            id = format!("{id}:{sidecar}");
        }
        let session = sessions
            .entry(id.clone())
            .or_insert_with(|| new_session("claude-code", id, path, "claude-code-jsonl-v1"));
        let message = record
            .get("message")
            .filter(|m| m.is_object())
            .context("Claude Code record is missing its message object")?;
        let content = text(message.get("content"));
        let mut role = string(message, "role").unwrap_or_else(|| kind.into());
        if record.get("isMeta").and_then(Value::as_bool) == Some(true) {
            role = "system".into();
        }
        if kind == "user"
            && (record
                .get("origin")
                .and_then(|o| o.get("kind"))
                .and_then(Value::as_str)
                .is_some_and(|origin| origin != "human")
                || content.trim_start().starts_with("<task-notification>"))
        {
            role = "agent".into();
        }
        if kind == "user"
            && (sidecar.is_some()
                || record.get("isSidechain").and_then(Value::as_bool) == Some(true))
        {
            role = "agent".into();
        }
        if kind == "user"
            && content.is_empty()
            && message
                .get("content")
                .and_then(Value::as_array)
                .is_some_and(|parts| {
                    parts
                        .iter()
                        .any(|p| p.get("type").and_then(Value::as_str) == Some("tool_result"))
                })
        {
            role = "tool".into();
        }
        if let Some(cwd) = record.get("cwd") {
            session.metadata["cwd"] = cwd.clone();
        }
        add_message(
            session,
            Message {
                source_id: string(record, "uuid").context("Claude Code message is missing uuid")?,
                parent_source_id: string(record, "parentUuid"),
                role,
                content,
                created_at: timestamp(record.get("timestamp"))?,
                metadata: record.clone(),
            },
        )?;
    }
    for record in records {
        let kind = record.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(kind, "custom-title" | "ai-title")
            && let Some(id) = string(record, "sessionId")
            && let Some(session) = sessions.get_mut(&id)
        {
            session.title = string(record, "customTitle")
                .or_else(|| string(record, "title"))
                .or_else(|| string(record, "aiTitle"))
                .or_else(|| session.title.take());
        }
    }
    Ok(sessions.into_values().collect())
}
