//! Explicit, read-only import of local coding-assistant session formats.
//! No credentials, application settings or unrelated browser data are scanned.
use crate::{
    import::read_chatgpt,
    model::{Conversation, Message, digest},
};
use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_BATCH_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFormat {
    Auto,
    Chatgpt,
    ClaudeCode,
    Codex,
    Kiro,
}

#[derive(Debug, Serialize)]
pub struct DetectedSource {
    pub provider: String,
    pub path: PathBuf,
    pub format: SourceFormat,
    pub files: usize,
    pub bytes: u64,
    pub collection_mode: String,
}

/// Inventory only the documented/session-specific locations. Detection does
/// not enable ingestion or background observation; import is a separate step.
pub fn detect_local(home: &Path) -> Result<Vec<DetectedSource>> {
    let locations = [
        (
            "claude-code",
            home.join(".claude/projects"),
            SourceFormat::ClaudeCode,
        ),
        ("codex", home.join(".codex/sessions"), SourceFormat::Codex),
        (
            "kiro",
            home.join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent"),
            SourceFormat::Kiro,
        ),
    ];
    let mut found = Vec::new();
    for (provider, path, format) in locations {
        if !path.is_dir() {
            continue;
        }
        let files = source_files(&path, format)?;
        if files.is_empty() {
            continue;
        }
        let bytes = files.iter().try_fold(0u64, |sum, p| -> Result<u64> {
            Ok(sum + fs::metadata(p)?.len())
        })?;
        found.push(DetectedSource {
            provider: provider.into(),
            path,
            format,
            files: files.len(),
            bytes,
            collection_mode: "manual local import".into(),
        });
    }
    Ok(found)
}

pub fn read_source(path: &Path, format: SourceFormat) -> Result<Vec<Conversation>> {
    let metadata = fs::symlink_metadata(path).context("inspect selected conversation source")?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "source root cannot be a symlink"
    );
    if metadata.is_file() && format == SourceFormat::Chatgpt {
        return read_chatgpt(path);
    }
    ensure!(
        format != SourceFormat::Chatgpt || !metadata.is_dir(),
        "ChatGPT import requires a JSON file or ZIP archive"
    );
    ensure!(
        !metadata.is_dir() || format != SourceFormat::Auto,
        "directory import requires --source claude-code, codex or kiro"
    );
    let files = if metadata.is_dir() {
        source_files(path, format)?
    } else {
        vec![path.to_path_buf()]
    };
    ensure!(
        !files.is_empty(),
        "selected location contains no supported conversation files"
    );
    let mut total = 0u64;
    let mut conversations = BTreeMap::<(String, String), Conversation>::new();
    for file in files {
        let size = fs::metadata(&file)?.len();
        total = total.checked_add(size).context("source size overflow")?;
        ensure!(
            size <= MAX_FILE_BYTES && total <= MAX_BATCH_BYTES,
            "local import exceeds 256 MiB per file or 512 MiB per batch"
        );
        let format = if format == SourceFormat::Auto {
            detect_format(&file)?
        } else {
            format
        };
        let imported = match format {
            SourceFormat::Chatgpt => read_chatgpt(&file)?,
            SourceFormat::ClaudeCode => parse_claude(&read_records(&file)?, &file)?,
            SourceFormat::Codex => parse_codex(&read_records(&file)?, &file)?,
            SourceFormat::Kiro => parse_kiro(
                &serde_json::from_slice(&read_bounded(&file)?).context("parse Kiro chat JSON")?,
                &file,
            )?,
            SourceFormat::Auto => unreachable!(),
        };
        for conversation in imported {
            let identity = (
                conversation.provider.clone(),
                conversation.source_id.clone(),
            );
            if let Some(previous) = conversations.get_mut(&identity) {
                if format == SourceFormat::Kiro {
                    merge_kiro_snapshot(previous, conversation)?;
                    continue;
                }
                ensure!(
                    previous == &conversation,
                    "conflicting session identities across selected files; import files separately to retain distinct revisions"
                );
            } else {
                conversations.insert(identity, conversation);
            }
        }
    }
    Ok(conversations.into_values().collect())
}

// Kiro saves cumulative snapshots under different filenames for one execution.
// Coalesce only exact message prefixes; divergent histories must be reviewed.
fn merge_kiro_snapshot(previous: &mut Conversation, incoming: Conversation) -> Result<()> {
    let common = previous.messages.len().min(incoming.messages.len());
    ensure!(
        previous.messages[..common] == incoming.messages[..common],
        "divergent Kiro snapshots share an execution ID; import files separately"
    );
    // These are per-generation times, rather than one stable chat timestamp.
    let created_at = [previous.created_at, incoming.created_at]
        .into_iter()
        .flatten()
        .min_by(f64::total_cmp);
    let updated_at = [previous.updated_at, incoming.updated_at]
        .into_iter()
        .flatten()
        .max_by(f64::total_cmp);
    let mut paths = Vec::new();
    let mut snapshots = BTreeMap::new();
    for metadata in [&previous.metadata, &incoming.metadata] {
        if let Some(path) = metadata.get("source_path").and_then(Value::as_str) {
            paths.push(path.to_owned());
            snapshots.insert(path.to_owned(), metadata["source_metadata"].clone());
        }
        if let Some(existing) = metadata.get("source_snapshots").and_then(Value::as_object) {
            snapshots.extend(
                existing
                    .iter()
                    .map(|(path, value)| (path.clone(), value.clone())),
            );
        }
        if let Some(existing) = metadata.get("source_paths").and_then(Value::as_array) {
            paths.extend(existing.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    paths.sort();
    paths.dedup();
    if incoming.messages.len() > previous.messages.len()
        || (incoming.messages.len() == previous.messages.len()
            && incoming.updated_at > previous.updated_at)
    {
        *previous = incoming;
    }
    previous
        .metadata
        .as_object_mut()
        .expect("normalized metadata object")
        .remove("source_path");
    previous.metadata["source_paths"] = json!(paths);
    previous.metadata["source_snapshots"] = json!(snapshots);
    previous.created_at = created_at;
    previous.updated_at = updated_at;
    Ok(())
}

fn source_files(root: &Path, format: SourceFormat) -> Result<Vec<PathBuf>> {
    let extension = match format {
        SourceFormat::ClaudeCode | SourceFormat::Codex => "jsonl",
        SourceFormat::Kiro => "chat",
        _ => bail!("unsupported source directory format"),
    };
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut visited = 0usize;
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory).context("read selected session directory")? {
            let entry = entry?;
            visited += 1;
            ensure!(
                visited <= 100_000,
                "selected source directory has too many entries"
            );
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() && entry.path().extension().is_some_and(|v| v == extension) {
                files.push(entry.path());
                ensure!(
                    files.len() <= MAX_FILES,
                    "selected source has more than 10000 session files"
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

fn detect_format(path: &Path) -> Result<SourceFormat> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("zip") {
        return Ok(SourceFormat::Chatgpt);
    }
    if extension.eq_ignore_ascii_case("chat") {
        return Ok(SourceFormat::Kiro);
    }
    let bytes = read_bounded(path)?;
    let first = bytes.iter().copied().find(|b| !b.is_ascii_whitespace());
    if first == Some(b'[') {
        return Ok(SourceFormat::Chatgpt);
    }
    if let Ok(value) = serde_json::from_slice::<Value>(&bytes)
        && value.get("chat").is_some_and(Value::is_array)
    {
        return Ok(SourceFormat::Kiro);
    }
    for line in bytes
        .split(|b| *b == b'\n')
        .filter(|s| !s.iter().all(u8::is_ascii_whitespace))
        .take(20)
    {
        if let Ok(record) = serde_json::from_slice::<Value>(line) {
            if record.get("sessionId").is_some() {
                return Ok(SourceFormat::ClaudeCode);
            }
            if record.get("type").and_then(Value::as_str) == Some("session_meta") {
                return Ok(SourceFormat::Codex);
            }
        }
    }
    bail!("unrecognized conversation format; select --source explicitly for a supported format")
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).context("open selected session transcript")?;
    ensure!(file.metadata()?.is_file(), "source must be a regular file");
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("read selected session transcript")?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "source exceeds 256 MiB file limit"
    );
    Ok(bytes)
}

fn read_records(path: &Path) -> Result<Vec<Value>> {
    let bytes = read_bounded(path)?;
    let mut records = Vec::new();
    for (index, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        ensure!(
            line.len() <= 16 * 1024 * 1024,
            "session record {} exceeds the 16 MiB line limit",
            index + 1
        );
        let record = serde_json::from_slice::<Value>(line)
            .with_context(|| format!("invalid session JSON at line {}", index + 1))?;
        ensure!(
            record.is_object(),
            "session line {} must contain an object",
            index + 1
        );
        records.push(record);
    }
    Ok(records)
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}
fn timestamp(value: Option<&Value>) -> Result<Option<f64>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_f64()
            .map(Some)
            .context("invalid numeric source timestamp"),
        Some(Value::String(value)) => Ok(Some(
            chrono::DateTime::parse_from_rfc3339(value)
                .context("invalid RFC3339 source timestamp")?
                .timestamp_micros() as f64
                / 1_000_000.0,
        )),
        _ => bail!("source timestamp must be numeric, RFC3339 text or null"),
    }
}
fn text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
                matches!(kind, "text" | "input_text" | "output_text")
                    .then(|| part.get("text").and_then(Value::as_str))
                    .flatten()
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
fn is_subagent(value: Option<&Value>) -> bool {
    value.is_some_and(|value| match value {
        Value::String(s) => matches!(s.as_str(), "subagent" | "sub_agent"),
        Value::Object(map) => {
            map.contains_key("subagent")
                || map.contains_key("sub_agent")
                || map
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.contains("subagent"))
        }
        _ => false,
    })
}
fn new_session(provider: &str, id: String, path: &Path, format: &str) -> Conversation {
    Conversation {
        provider: provider.into(),
        source_id: id,
        title: None,
        created_at: None,
        updated_at: None,
        metadata: json!({"source_format":format,"source_path":path.to_string_lossy()}),
        messages: Vec::new(),
    }
}
fn add_message(conversation: &mut Conversation, message: Message) -> Result<()> {
    if let Some(previous) = conversation
        .messages
        .iter()
        .find(|m| m.source_id == message.source_id)
    {
        ensure!(
            previous == &message,
            "source message ID has conflicting records"
        );
        return Ok(());
    }
    if let Some(time) = message.created_at {
        conversation.updated_at = Some(
            conversation
                .updated_at
                .map_or(time, |current| current.max(time)),
        );
    }
    conversation.messages.push(message);
    Ok(())
}

fn parse_claude(records: &[Value], path: &Path) -> Result<Vec<Conversation>> {
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

fn parse_codex(records: &[Value], path: &Path) -> Result<Vec<Conversation>> {
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

fn parse_kiro(record: &Value, path: &Path) -> Result<Vec<Conversation>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_generated_user_role_records_cannot_become_personal_facts() -> Result<()> {
        let claude = parse_claude(&[
            json!({"type":"user","sessionId":"test","uuid":"notification","origin":{"kind":"task-notification"},"message":{"role":"user","content":"Do not buy an expensive product."}}),
            json!({"type":"user","sessionId":"test","uuid":"human","origin":{"kind":"human"},"message":{"role":"user","content":"I prefer Rust."}}),
        ],Path::new("synthetic.jsonl"))?.remove(0);
        assert_eq!(claude.messages[0].role, "agent");
        assert_eq!(
            crate::intelligence::extract_rules(&claude, 1)
                .observations
                .len(),
            1
        );
        let codex = parse_codex(&[
            json!({"type":"session_meta","payload":{"id":"test"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<codex_internal_context source=\"goal\">\nDo not invent user preferences.\n</codex_internal_context>"}]}}),
        ],Path::new("synthetic.jsonl"))?.remove(0);
        assert_eq!(codex.messages[0].role, "system");
        assert!(
            crate::intelligence::extract_rules(&codex, 1)
                .observations
                .is_empty()
        );
        Ok(())
    }
    #[test]
    fn kiro_provider_prompt_and_environment_are_not_human_preferences() -> Result<()> {
        let record = json!({"executionId":"test","chat":[
            {"role":"human","content":"<identity>\nYou are Kiro, an AI assistant.\n</identity>\nDo not add tests."},
            {"role":"human","content":"I prefer Rust.\n<EnvironmentContext>\nI prefer fabricated environment instructions.\n</EnvironmentContext>"}
        ]});
        let source = parse_kiro(&record, Path::new("synthetic.chat"))?.remove(0);
        assert_eq!(source.messages[0].role, "system");
        assert_eq!(source.messages[1].content, "I prefer Rust.");
        assert_eq!(source.messages[1].metadata, record["chat"][1]);
        let output = crate::intelligence::extract_rules(&source, 1);
        assert_eq!(output.observations.len(), 1);
        assert_eq!(output.observations[0].canonical_value, "prefer rust");
        Ok(())
    }
    #[test]
    fn inherited_codex_metadata_cannot_reassign_child_messages_to_human_parent() -> Result<()> {
        let records = vec![
            json!({"type":"session_meta","payload":{"id":"child","source":{"subagent":{}},"thread_source":"subagent"}}),
            json!({"type":"session_meta","payload":{"id":"parent","source":"vscode","thread_source":"user"}}),
            json!({"type":"response_item","ordinal":4,"payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer fabricated agent requests."}]}}),
        ];
        let sources = parse_codex(&records, Path::new("child.jsonl"))?;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_id, "child");
        assert_eq!(sources[0].messages[0].role, "agent");
        assert!(
            crate::intelligence::extract_rules(&sources[0], 1)
                .observations
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn kiro_directory_coalesces_exact_prefixes_but_refuses_divergent_snapshots() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let first = json!({"executionId":"test","metadata":{"startTime":1000,"endTime":2000},"chat":[{"role":"human","content":"I prefer Rust."}]});
        let mut second = first.clone();
        second["chat"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"bot","content":"Okay"}));
        second["metadata"]["endTime"] = json!(3000);
        second["metadata"]["startTime"] = json!(2500);
        fs::write(
            directory.path().join("first.chat"),
            serde_json::to_vec(&first)?,
        )?;
        fs::write(
            directory.path().join("second.chat"),
            serde_json::to_vec(&second)?,
        )?;
        let sources = read_source(directory.path(), SourceFormat::Kiro)?;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].messages.len(), 2);
        assert_eq!(sources[0].updated_at, Some(3.0));
        assert_eq!(sources[0].created_at, Some(1.0));
        assert_eq!(
            sources[0].metadata["source_paths"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(read_source(directory.path(), SourceFormat::Kiro)?, sources);
        second["chat"][0]["content"] = json!("A different history");
        fs::write(
            directory.path().join("second.chat"),
            serde_json::to_vec(&second)?,
        )?;
        assert!(read_source(directory.path(), SourceFormat::Kiro).is_err());
        Ok(())
    }
    #[test]
    fn claude_preserves_ids_times_and_keeps_tools_and_agent_requests_out_of_user_facts()
    -> Result<()> {
        let records = vec![
            json!({"type":"user","sessionId":"test","uuid":"m1","timestamp":"2026-10-19T10:00:00+02:00","message":{"role":"user","content":"I prefer Rust."}}),
            json!({"type":"user","sessionId":"test","uuid":"m2","parentUuid":"m1","isSidechain":true,"message":{"role":"user","content":"I prefer invented agent instructions."}}),
            json!({"type":"user","sessionId":"test","uuid":"m3","message":{"role":"user","content":[{"type":"tool_result","content":"I prefer tool output"}]}}),
        ];
        let source = parse_claude(&records, Path::new("synthetic.jsonl"))?.remove(0);
        assert_eq!(source.messages.len(), 3);
        assert_eq!(source.messages[0].created_at, Some(1792396800.0));
        assert_eq!(source.messages[1].role, "agent");
        assert_eq!(source.messages[2].role, "tool");
        assert_eq!(
            crate::intelligence::extract_rules(&source, 1)
                .observations
                .len(),
            1
        );
        Ok(())
    }
    #[test]
    fn codex_ignores_duplicate_event_text_and_provider_instructions() -> Result<()> {
        let records = vec![
            json!({"type":"session_meta","payload":{"id":"test","timestamp":"2026-10-19T08:00:00Z","source":"cli"}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"I prefer Rust."}}),
            json!({"type":"response_item","ordinal":1,"payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"I prefer fake instructions"}]}}),
            json!({"type":"response_item","ordinal":2,"timestamp":"2026-10-19T08:01:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer Rust."}]}}),
        ];
        let source = parse_codex(&records, Path::new("synthetic.jsonl"))?.remove(0);
        assert_eq!(source.messages.len(), 2);
        assert_eq!(source.messages[1].source_id, "ordinal-2");
        assert_eq!(
            crate::intelligence::extract_rules(&source, 1)
                .observations
                .len(),
            1
        );
        Ok(())
    }
    #[test]
    fn kiro_retains_unknown_message_dates_and_stable_fallback_identities() -> Result<()> {
        let source = json!({"executionId":"test","metadata":{"startTime":1792396800000u64,"endTime":1792396860000u64},"chat":[{"role":"human","content":"Keep code simple."},{"role":"bot","content":"Okay"},{"role":"human","content":"Keep code simple."}]});
        let first = parse_kiro(&source, Path::new("synthetic.chat"))?.remove(0);
        assert_eq!(first.created_at, Some(1792396800.0));
        assert!(first.messages.iter().all(|m| m.created_at.is_none()));
        assert_ne!(first.messages[0].source_id, first.messages[2].source_id);
        let mut appended = source;
        appended["chat"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"bot","content":"New answer"}));
        let second = parse_kiro(&appended, Path::new("synthetic.chat"))?.remove(0);
        assert_eq!(first.messages, second.messages[..3]);
        Ok(())
    }
    #[test]
    fn discovery_import_is_read_only_and_directory_scanning_ignores_symlinks() -> Result<()> {
        let home = tempfile::tempdir()?;
        let directory = home.path().join(".claude/projects/project");
        fs::create_dir_all(&directory)?;
        let record=json!({"type":"user","sessionId":"test","uuid":"m1","message":{"role":"user","content":"I prefer Rust."}}).to_string();
        let path = directory.join("test.jsonl");
        fs::write(&path, &record)?;
        fs::write(
            directory.join("unrelated.credentials"),
            "must never be read",
        )?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&path, directory.join("duplicate.jsonl"))?;
        let detected = detect_local(home.path())?;
        assert_eq!(detected[0].files, 1);
        let before = fs::read(&path)?;
        let imported = read_source(&directory, SourceFormat::ClaudeCode)?;
        assert_eq!(imported.len(), 1);
        assert_eq!(fs::read(path)?, before);
        assert!(read_source(&directory, SourceFormat::Auto).is_err());
        Ok(())
    }
    #[test]
    fn malformed_ids_and_conflicting_message_records_fail_instead_of_dropping_evidence() {
        assert!(parse_kiro(&json!({"chat":[]}), Path::new("synthetic.chat")).is_err());
        assert!(
            parse_claude(
                &[json!({"type":"user","message":{"content":"hello"}})],
                Path::new("synthetic.jsonl")
            )
            .is_err()
        );
        assert!(
            parse_codex(
                &[json!({"type":"response_item","payload":{"type":"message","role":"user"}})],
                Path::new("synthetic.jsonl")
            )
            .is_err()
        );
    }
}
