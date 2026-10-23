//! Read the selected Codex desktop history databases without changing them.
use std::{collections::BTreeMap, fs, path::Path, time::Duration};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{Value, json};

use super::{MAX_BATCH_BYTES, MAX_FILES, SourceFormat};
use crate::model::{Conversation, Message};

#[derive(Debug, Default, Serialize)]
pub struct DesktopCoverage {
    pub indexed_threads: usize,
    pub threads_with_history: usize,
    pub threads_without_history: usize,
    pub worker_threads: usize,
    pub rollout_sources: usize,
    pub scope: &'static str,
}

pub struct DesktopImport {
    pub conversations: Vec<Conversation>,
    pub coverage: DesktopCoverage,
}

fn charge_input(bytes: &mut u64, size: i64) -> Result<()> {
    let size = u64::try_from(size).context("invalid desktop record size")?;
    *bytes = bytes.checked_add(size).context("source size overflow")?;
    ensure!(
        size <= 16 * 1024 * 1024 && *bytes <= MAX_BATCH_BYTES,
        "Codex desktop import exceeds 16 MiB per record or 512 MiB input budget"
    );
    Ok(())
}

fn validate_root(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "select a regular Codex home directory, not a symlink"
    );
    Ok(())
}

fn open(root: &Path, name: &str) -> Result<Connection> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path).with_context(|| {
        format!("Codex desktop import requires {name} in the selected directory")
    })?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Codex desktop databases must be regular files, not symlinks"
    );
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON; BEGIN DEFERRED")?;
    Ok(connection)
}

pub(super) fn worker(source: &Value, thread_source: Option<&str>) -> bool {
    super::is_subagent(Some(source))
        || matches!(thread_source, Some("subagent" | "guardian_review"))
}

/// Metadata-only inventory. Counts describe local Codex storage, not the whole app account.
pub fn inspect(root: &Path) -> Result<DesktopCoverage> {
    validate_root(root)?;
    let state = open(root, "state_5.sqlite")?;
    let history = open(root, "thread_history_1.sqlite")?;
    let mut coverage = DesktopCoverage {
        scope: "Local Codex desktop history only; ChatGPT account chats and remote-only tasks are not included.",
        ..DesktopCoverage::default()
    };
    let mut statement = state
        .prepare("SELECT id, source, thread_source,
            length(CAST(id AS BLOB)) + length(CAST(source AS BLOB)) + coalesce(length(CAST(thread_source AS BLOB)),0)
            FROM threads ORDER BY id LIMIT 10001")
        .context("unsupported Codex desktop metadata schema")?;
    let mut rows = statement.query([])?;
    let mut available = history.prepare("SELECT EXISTS(SELECT 1 FROM thread_items WHERE thread_id=?1 UNION ALL SELECT 1 FROM thread_realtime_items WHERE thread_id=?1)")
        .context("unsupported Codex desktop history schema")?;
    let mut bytes = 0;
    while let Some(row) = rows.next()? {
        charge_input(&mut bytes, row.get(3)?)?;
        coverage.indexed_threads += 1;
        ensure!(
            coverage.indexed_threads <= MAX_FILES,
            "Codex desktop index exceeds 10000 threads"
        );
        let id: String = row.get(0)?;
        let source: String = row.get(1)?;
        let source = serde_json::from_str(&source).unwrap_or(Value::String(source));
        let thread_source: Option<String> = row.get(2)?;
        coverage.worker_threads += usize::from(worker(&source, thread_source.as_deref()));
        if available.query_row([id], |row| row.get::<_, bool>(0))? {
            coverage.threads_with_history += 1;
        } else {
            coverage.threads_without_history += 1;
        }
    }
    Ok(coverage)
}

// shortcut: observed Codex v0.145 schemas; add an adapter when its storage changes.
pub fn read(root: &Path) -> Result<DesktopImport> {
    validate_root(root)?;
    let state = open(root, "state_5.sqlite")?;
    let history = open(root, "thread_history_1.sqlite")?;
    let mut coverage = DesktopCoverage {
        scope: "Local Codex desktop history only; ChatGPT account chats and remote-only tasks are not included.",
        ..DesktopCoverage::default()
    };
    // Prefer full local rollouts on overlap; partial projections must never replace them.
    let mut conversations = BTreeMap::<String, Conversation>::new();
    let mut bytes = 0u64;
    for directory in ["sessions", "archived_sessions"] {
        let path = root.join(directory);
        if !path.exists() {
            continue;
        }
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "session directories cannot be symlinks"
        );
        let files = super::source_files(&path, SourceFormat::Codex)?;
        if files.is_empty() {
            continue;
        }
        for file in files {
            bytes = bytes
                .checked_add(fs::metadata(file)?.len())
                .context("source size overflow")?;
        }
        ensure!(
            bytes <= MAX_BATCH_BYTES,
            "Codex desktop import exceeds 512 MiB input budget"
        );
        for source in super::read_source(&path, SourceFormat::Codex)? {
            let id = source.source_id.clone();
            if let Some(previous) = conversations.get(&id) {
                ensure!(
                    previous == &source,
                    "conflicting active/archived rollout identities"
                );
            } else {
                conversations.insert(id, source);
            }
        }
    }
    ensure!(
        conversations.len() <= MAX_FILES,
        "Codex desktop import exceeds 10000 sources"
    );
    coverage.rollout_sources = conversations.len();
    let mut threads = state.prepare("SELECT id, title, cwd, source, thread_source, created_at, updated_at, archived,
            length(CAST(id AS BLOB)) + coalesce(length(CAST(title AS BLOB)),0) + length(CAST(cwd AS BLOB))
            + length(CAST(source AS BLOB)) + coalesce(length(CAST(thread_source AS BLOB)),0)
            FROM threads ORDER BY id LIMIT 10001")
        .context("unsupported Codex desktop metadata schema")?;
    let mut rows = threads.query([])?;
    let mut items = history.prepare("SELECT turn_id, item_id, rollout_ordinal, item_json, length(CAST(item_json AS BLOB)) + length(CAST(item_id AS BLOB)) + coalesce(length(CAST(turn_id AS BLOB)),0) FROM thread_items WHERE thread_id=?1 AND item_type IN ('userMessage','agentMessage','') UNION ALL SELECT NULL, item_id, rollout_ordinal, item_json, length(CAST(item_json AS BLOB)) + length(CAST(item_id AS BLOB)) FROM thread_realtime_items WHERE thread_id=?1 AND item_type IN ('userMessage','agentMessage','') ORDER BY 3,2")
        .context("unsupported Codex desktop history schema")?;
    let mut present = history.prepare("SELECT EXISTS(SELECT 1 FROM thread_items WHERE thread_id=?1 UNION ALL SELECT 1 FROM thread_realtime_items WHERE thread_id=?1)")?;
    while let Some(row) = rows.next()? {
        charge_input(&mut bytes, row.get(8)?)?;
        coverage.indexed_threads += 1;
        ensure!(
            coverage.indexed_threads <= MAX_FILES,
            "Codex desktop index exceeds 10000 threads"
        );
        let id: String = row.get(0)?;
        let raw_source: String = row.get(3)?;
        let source = serde_json::from_str(&raw_source).unwrap_or(Value::String(raw_source));
        let thread_source: Option<String> = row.get(4)?;
        let is_worker = worker(&source, thread_source.as_deref());
        coverage.worker_threads += usize::from(is_worker);
        let has_history = present.query_row([&id], |row| row.get::<_, bool>(0))?;
        if has_history {
            coverage.threads_with_history += 1;
        } else {
            coverage.threads_without_history += 1;
        }
        if let Some(rollout) = conversations.get_mut(&id) {
            // The desktop index can identify review workers absent from older rollout metadata.
            if is_worker {
                rollout.metadata["thread_source"] = json!(thread_source);
                rollout.metadata["source"] = source;
                for message in &mut rollout.messages {
                    if message.role == "user" {
                        message.role = "agent".into();
                    }
                }
            }
            continue;
        }
        if !has_history {
            continue;
        }
        let owner = !is_worker
            && (matches!(thread_source.as_deref(), Some("user" | "realtime_voice"))
                || (thread_source.is_none()
                    && source
                        .as_str()
                        .is_some_and(|s| matches!(s, "cli" | "vscode" | "exec"))));
        let mut conversation = Conversation {
            provider: "codex".into(),
            source_id: id.clone(),
            title: row.get(1)?,
            created_at: row.get::<_, Option<i64>>(5)?.map(|n| n as f64),
            updated_at: row.get::<_, Option<i64>>(6)?.map(|n| n as f64),
            messages: Vec::new(),
            metadata: json!({"source_format":"codex-desktop-history-v1", "source_path":std::path::absolute(root)?.to_string_lossy(),
                "cwd":row.get::<_,String>(2)?,"source":source,"thread_source":thread_source,"archived":row.get::<_,bool>(7)?,
                "remnary_collection":{"format":"codex-desktop-history-v1","partial":true,"message_timestamps":"unavailable","coverage":"Persisted local message projection; tool items and unavailable account history are omitted."}}),
        };
        let mut records = items.query([&id])?;
        let mut seen = BTreeMap::<String, Message>::new();
        while let Some(item) = records.next()? {
            charge_input(&mut bytes, item.get(4)?)?;
            let raw: String = item.get(3)?;
            let value: Value =
                serde_json::from_str(&raw).context("malformed Codex desktop item JSON")?;
            let kind = value
                .get("type")
                .and_then(Value::as_str)
                .context("desktop item type missing")?;
            let (role, content) = match kind {
                "userMessage" => {
                    let parts = value
                        .get("content")
                        .and_then(Value::as_array)
                        .context("desktop user item needs content parts")?;
                    let mut text = Vec::new();
                    for part in parts {
                        if part.get("type").and_then(Value::as_str) == Some("text") {
                            text.push(
                                part.get("text")
                                    .and_then(Value::as_str)
                                    .context("desktop text part needs text")?,
                            );
                        }
                    }
                    (
                        if is_worker {
                            "agent"
                        } else if owner {
                            "user"
                        } else {
                            "unattributed_user"
                        },
                        text.join("\n"),
                    )
                }
                "agentMessage" => (
                    "assistant",
                    value
                        .get("text")
                        .and_then(Value::as_str)
                        .context("desktop assistant item needs text")?
                        .to_owned(),
                ),
                _ => continue,
            };
            let message_id: String = item.get(1)?;
            ensure!(
                !message_id.is_empty()
                    && value.get("id").and_then(Value::as_str) == Some(message_id.as_str()),
                "desktop item identity mismatch"
            );
            let message = Message {
                source_id: message_id.clone(),
                parent_source_id: None,
                role: if role == "user"
                    && value.get("truncated").and_then(Value::as_bool) == Some(true)
                {
                    "truncated_user"
                } else {
                    role
                }
                .into(),
                content,
                created_at: None,
                metadata: json!({"app_item":value,"turn_id":item.get::<_,Option<String>>(0)?,"rollout_ordinal":item.get::<_,i64>(2)?}),
            };
            if let Some(previous) = seen.get(&message_id) {
                ensure!(
                    previous == &message,
                    "conflicting duplicate desktop item identity"
                );
            } else {
                ensure!(
                    seen.len() < 100_000,
                    "desktop thread exceeds 100000 messages"
                );
                seen.insert(message_id, message.clone());
                conversation.messages.push(message);
            }
        }
        conversations.insert(id, conversation);
        ensure!(
            conversations.len() <= MAX_FILES,
            "Codex desktop import exceeds 10000 sources"
        );
    }
    Ok(DesktopImport {
        conversations: conversations.into_values().collect(),
        coverage,
    })
}

#[cfg(test)]
mod tests;
