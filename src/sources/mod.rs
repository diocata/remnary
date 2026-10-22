//! Explicit, read-only import of local coding-assistant session formats.
//! No credentials, application settings or unrelated browser data are scanned.
pub mod app_snapshot;
pub mod chatgpt;
pub(crate) mod origins;

mod claude;
mod codex;
mod kiro;

use claude::parse_claude;
use codex::parse_codex;
use kiro::parse_kiro;

use crate::{
    import::read_chatgpt,
    model::{Conversation, Message},
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
fn kiro_session_directory(home: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = home.join("AppData/Roaming");
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = home.join(".config");
    base.join("Kiro/User/globalStorage/kiro.kiroagent")
}

pub fn detect_local(home: &Path) -> Result<Vec<DetectedSource>> {
    let locations = [
        (
            "claude-code",
            home.join(".claude/projects"),
            SourceFormat::ClaudeCode,
        ),
        ("codex", home.join(".codex/sessions"), SourceFormat::Codex),
        ("kiro", kiro_session_directory(home), SourceFormat::Kiro),
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
        metadata: json!({"source_format":format,"source_path":std::path::absolute(path).unwrap_or_else(|_|path.to_path_buf()).to_string_lossy()}),
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

#[cfg(test)]
mod tests;
