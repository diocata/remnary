use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

use crate::model::{Conversation, Message};

const MAX_IMPORT_BYTES: u64 = 256 * 1024 * 1024;

/// Read an extracted ChatGPT `conversations.json` or the original export ZIP (one file or numbered shards).
///
/// All mapping branches are retained, with parents preceding their children.
/// ZIP members are read directly; archive paths are never extracted to disk.
/// Graph metadata stays in `Conversation.metadata.mapping`, while original
/// non-null message objects are kept once in each `Message.metadata`.
pub fn read_chatgpt(path: &Path) -> Result<Vec<Conversation>> {
    let mut file =
        File::open(path).with_context(|| format!("open ChatGPT export {}", path.display()))?;
    ensure!(
        file.metadata()?.len() <= MAX_IMPORT_BYTES,
        "ChatGPT export exceeds the {} MiB input limit",
        MAX_IMPORT_BYTES / 1024 / 1024
    );

    let mut magic = [0; 4];
    let count = file.read(&mut magic).context("read export file header")?;
    file.seek(SeekFrom::Start(0))?;
    let zip_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    let zip_magic = count == 4
        && matches!(
            magic,
            [b'P', b'K', 3, 4] | [b'P', b'K', 5, 6] | [b'P', b'K', 7, 8]
        );

    let conversations = if zip_extension || zip_magic {
        read_zip(file)
    } else {
        let bytes = read_bounded(file, MAX_IMPORT_BYTES).context("read conversations.json")?;
        parse_chatgpt(&bytes)
    };
    conversations.with_context(|| format!("import ChatGPT export {}", path.display()))
}

fn read_zip(file: File) -> Result<Vec<Conversation>> {
    read_zip_with_limit(file, MAX_IMPORT_BYTES)
}

fn read_zip_with_limit(file: File, limit: u64) -> Result<Vec<Conversation>> {
    let mut archive = zip::ZipArchive::new(file).context("open ChatGPT ZIP archive")?;
    ensure!(
        archive.len() <= 100_000,
        "ChatGPT ZIP exceeds the 100,000-member limit"
    );
    let mut legacy = Vec::new();
    let mut shards = BTreeMap::new();
    let mut folder = None;
    let mut declared_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).context("read ChatGPT ZIP member")?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().context("ZIP entry name is not valid text")?;
        let (parent, basename) = name.rsplit_once('/').unwrap_or(("", name.as_ref()));
        let shard = basename
            .strip_prefix("conversations-")
            .and_then(|s| s.strip_suffix(".json"))
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
        if basename != "conversations.json" && shard.is_none() {
            continue;
        }
        if let Some(previous) = &folder {
            ensure!(
                previous == parent,
                "ChatGPT ZIP contains ambiguous conversation folders; found 2 or more"
            );
        } else {
            folder = Some(parent.to_owned());
        }
        declared_bytes = declared_bytes
            .checked_add(entry.size())
            .context("ZIP expanded size overflow")?;
        ensure!(
            declared_bytes <= limit,
            "ZIP conversation files exceed the {limit}-byte total uncompressed input limit"
        );
        if let Some(number) = shard {
            let number: u64 = number
                .parse()
                .context("conversation shard number is too large")?;
            ensure!(
                shards.insert(number, index).is_none(),
                "ChatGPT ZIP contains duplicate conversation shard numbers"
            );
            ensure!(
                shards.len() <= 1024,
                "ChatGPT ZIP exceeds the 1,024-shard limit"
            );
        } else {
            legacy.push(index);
        }
    }
    ensure!(
        (legacy.len() == 1 && shards.is_empty()) || (legacy.is_empty() && !shards.is_empty()),
        "ChatGPT ZIP must contain one conversations.json or numbered conversations-*.json shards, never both; found {} legacy files and {} shards",
        legacy.len(),
        shards.len()
    );
    let indexes: Vec<_> = if legacy.is_empty() {
        shards.into_values().collect()
    } else {
        legacy
    };
    let mut consumed = 0;
    let mut conversations = Vec::<Conversation>::new();
    let mut seen = BTreeMap::<String, usize>::new();
    for index in indexes {
        let entry = archive
            .by_index(index)
            .context("open conversation file in ZIP")?;
        // Cap actual streamed bytes too, independently of declared ZIP sizes.
        let bytes =
            read_bounded(entry, limit - consumed).context("read conversation file in ZIP")?;
        consumed += bytes.len() as u64;
        for conversation in parse_chatgpt(&bytes)? {
            if let Some(previous) = seen.get(&conversation.source_id) {
                ensure!(
                    conversations[*previous] == conversation,
                    "conflicting duplicate conversation ID across ZIP shards"
                );
            } else {
                seen.insert(conversation.source_id.clone(), conversations.len());
            }
            conversations.push(conversation);
        }
    }
    Ok(conversations)
}

fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "input exceeds the {limit}-byte uncompressed input limit"
    );
    Ok(bytes)
}

fn parse_chatgpt(bytes: &[u8]) -> Result<Vec<Conversation>> {
    let document: Value = serde_json::from_slice(bytes).context("parse conversations.json")?;
    let records = document
        .as_array()
        .context("conversations.json must contain an array of conversation objects")?;
    let mut conversations = Vec::with_capacity(records.len());
    let mut seen = BTreeMap::<String, &Value>::new();
    for (index, record) in records.iter().enumerate() {
        let conversation = parse_conversation(record)
            .with_context(|| format!("invalid ChatGPT conversation at array index {index}"))?;
        if let Some(previous) = seen.get(&conversation.source_id) {
            ensure!(
                *previous == record,
                "conflicting duplicate conversation ID {:?} at array index {index}",
                conversation.source_id
            );
        } else {
            seen.insert(conversation.source_id.clone(), record);
        }
        conversations.push(conversation);
    }
    Ok(conversations)
}

fn parse_conversation(record: &Value) -> Result<Conversation> {
    let object = record
        .as_object()
        .context("expected a conversation object")?;
    let source_id = optional_string(object, "id")?
        .or(optional_string(object, "conversation_id")?)
        .context("conversation has no stable id or conversation_id")?;
    ensure!(
        !source_id.trim().is_empty(),
        "conversation id must not be empty"
    );
    let mapping = object
        .get("mapping")
        .and_then(Value::as_object)
        .with_context(|| format!("conversation {source_id:?} must have a mapping object"))?;
    let messages =
        parse_mapping(mapping).with_context(|| format!("conversation {source_id:?} mapping"))?;
    let mut metadata: Map<String, Value> = object
        .iter()
        .filter(|(key, _)| key.as_str() != "mapping")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    // Keep the original graph and node-specific fields, including null-message
    // roots/bridges. Non-null messages live in Message.metadata, so retaining
    // their text again inside this graph would duplicate the full export.
    let graph = mapping
        .iter()
        .map(|(id, value)| {
            let node = value
                .as_object()
                .expect("mapping nodes were validated")
                .iter()
                .filter(|(key, value)| key.as_str() != "message" || value.is_null())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            (id.clone(), Value::Object(node))
        })
        .collect();
    metadata.insert("mapping".to_owned(), Value::Object(graph));

    Ok(Conversation {
        provider: "chatgpt".to_owned(),
        source_id,
        title: optional_string(object, "title")?,
        created_at: optional_timestamp(object, "create_time")?,
        updated_at: optional_timestamp(object, "update_time")?,
        metadata: Value::Object(metadata),
        messages,
    })
}

struct MappingNode {
    parent: Option<String>,
    message: Option<Message>,
}

fn parse_mapping(mapping: &Map<String, Value>) -> Result<Vec<Message>> {
    let mut nodes = BTreeMap::new();
    for (source_id, value) in mapping {
        ensure!(
            !source_id.trim().is_empty(),
            "mapping node ID must not be empty"
        );
        let node = parse_node(source_id, value)
            .with_context(|| format!("invalid mapping node {source_id:?}"))?;
        nodes.insert(source_id.clone(), node);
    }

    let mut ready = BTreeSet::new();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for (source_id, node) in &nodes {
        if let Some(parent) = &node.parent {
            ensure!(
                nodes.contains_key(parent),
                "mapping node {source_id:?} references missing parent {parent:?}"
            );
            children
                .entry(parent.clone())
                .or_default()
                .push(source_id.clone());
        } else {
            ready.insert(source_id.clone());
        }
    }

    // An iterative topological traversal avoids recursion on long conversations.
    // Lexical ordering makes siblings and multiple roots deterministic.
    let mut messages = Vec::new();
    while let Some(source_id) = ready.pop_first() {
        let node = nodes.remove(&source_id).expect("ready node exists");
        if let Some(message) = node.message {
            messages.push(message);
        }
        if let Some(descendants) = children.remove(&source_id) {
            ready.extend(descendants);
        }
    }
    if !nodes.is_empty() {
        let ids: Vec<_> = nodes.keys().take(5).collect();
        bail!("mapping contains a parent cycle involving {ids:?}");
    }
    Ok(messages)
}

fn parse_node(source_id: &str, value: &Value) -> Result<MappingNode> {
    let node = value
        .as_object()
        .context("expected a mapping node object")?;
    let parent = optional_string(node, "parent")?;
    let message = node.get("message").context("missing message field")?;
    let message = if message.is_null() {
        None
    } else {
        let object = message
            .as_object()
            .context("message must be an object or null")?;
        let role = object
            .get("author")
            .and_then(|author| author.get("role"))
            .and_then(Value::as_str)
            .filter(|role| !role.is_empty())
            .unwrap_or("unknown")
            .to_owned();
        Some(Message {
            source_id: source_id.to_owned(),
            parent_source_id: parent.clone(),
            role,
            content: message_text(object.get("content"))?,
            created_at: optional_timestamp(object, "create_time")?,
            metadata: message.clone(),
        })
    };
    Ok(MappingNode { parent, message })
}

fn message_text(content: Option<&Value>) -> Result<String> {
    let Some(content) = content.and_then(Value::as_object) else {
        // Unknown content forms remain available in the original metadata.
        return Ok(String::new());
    };
    if let Some(parts) = content.get("parts") {
        let parts = parts
            .as_array()
            .context("message content.parts must be an array")?;
        Ok(parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        // Code and execution-output messages can use `text` instead of `parts`.
        Ok(content
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Result<Option<String>> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => bail!("{key} must be a string or null"),
    }
}

fn optional_timestamp(object: &Map<String, Value>, key: &str) -> Result<Option<f64>> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .map(Some)
            .with_context(|| format!("{key} must be a numeric Unix timestamp or null")),
    }
}

#[cfg(test)]
mod tests;
