use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

use crate::model::{Conversation, Message};

const MAX_IMPORT_BYTES: u64 = 256 * 1024 * 1024;

/// Read an extracted ChatGPT `conversations.json` or the original export ZIP.
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

    let bytes = if zip_extension || zip_magic {
        read_zip(file)?
    } else {
        read_bounded(file, MAX_IMPORT_BYTES).context("read conversations.json")?
    };
    parse_chatgpt(&bytes).with_context(|| format!("import ChatGPT export {}", path.display()))
}

fn read_zip(file: File) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(file).context("open ChatGPT ZIP archive")?;
    let mut matches = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .with_context(|| format!("read ZIP member {index}"))?;
        if entry.is_file() && entry.name().rsplit('/').next() == Some("conversations.json") {
            matches.push(index);
        }
    }
    ensure!(
        matches.len() == 1,
        "ChatGPT ZIP must contain exactly one conversations.json; found {}",
        matches.len()
    );
    let entry = archive
        .by_index(matches[0])
        .context("open conversations.json in ZIP")?;
    ensure!(
        entry.size() <= MAX_IMPORT_BYTES,
        "ZIP conversations.json exceeds the {} MiB uncompressed input limit",
        MAX_IMPORT_BYTES / 1024 / 1024
    );
    // The streaming cap also applies if an archive's declared size is incorrect.
    read_bounded(entry, MAX_IMPORT_BYTES).context("read conversations.json in ZIP")
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
mod tests {
    use std::io::{Cursor, Write};

    use serde_json::json;
    use zip::write::SimpleFileOptions;

    use super::*;

    const BRANCHES: &str = include_str!("../tests/fixtures/chatgpt-branches.json");

    fn parse(value: Value) -> Result<Vec<Conversation>> {
        parse_chatgpt(&serde_json::to_vec(&value)?)
    }

    fn record() -> Value {
        json!({
            "id": "conversation-1",
            "mapping": {
                "root": {"parent": null, "message": null},
                "user": {
                    "parent": "root",
                    "message": {
                        "author": {"role": "user"},
                        "content": {"content_type": "text", "parts": ["Hello"]}
                    }
                }
            }
        })
    }

    #[test]
    fn preserves_every_branch_in_parent_before_child_order() -> Result<()> {
        let imported = parse_chatgpt(BRANCHES.as_bytes())?;
        let conversation = &imported[0];
        assert_eq!(conversation.provider, "chatgpt");
        assert_eq!(conversation.source_id, "conversation-branches");
        assert_eq!(
            conversation
                .messages
                .iter()
                .map(|message| message.source_id.as_str())
                .collect::<Vec<_>>(),
            ["b-user", "a-alternative", "c-primary", "d-followup"]
        );
        assert_eq!(
            conversation.messages[0].parent_source_id.as_deref(),
            Some("root")
        );
        assert_eq!(
            conversation.messages[1].parent_source_id.as_deref(),
            Some("b-user")
        );
        assert_eq!(
            conversation.messages[2].parent_source_id.as_deref(),
            Some("b-user")
        );
        assert_eq!(
            conversation.messages[3].parent_source_id.as_deref(),
            Some("c-primary")
        );
        // The current branch does not determine which messages are imported.
        assert_eq!(conversation.metadata["current_node"], "a-alternative");
        assert_eq!(
            conversation.metadata["mapping"]["root"]["message"],
            Value::Null
        );
        assert!(
            conversation.metadata["mapping"]["b-user"]
                .get("message")
                .is_none()
        );
        assert_eq!(
            conversation.metadata["mapping"]["b-user"]["children"],
            json!(["c-primary", "a-alternative"])
        );
        Ok(())
    }

    #[test]
    fn keeps_unknown_metadata_and_multimodal_parts() -> Result<()> {
        let imported = parse_chatgpt(BRANCHES.as_bytes())?;
        let conversation = &imported[0];
        let message = &conversation.messages[0];
        assert_eq!(message.content, "Hello\nworld");
        assert_eq!(message.metadata["id"], "provider-message-id");
        assert_eq!(
            message.metadata["content"]["parts"][1]["asset_pointer"],
            "file-service://image"
        );
        assert_eq!(conversation.metadata["custom_field"], json!({"keep": true}));
        assert_eq!(
            conversation.metadata["mapping"]["root"]["custom_node_field"],
            json!({"preserve": true})
        );
        Ok(())
    }

    #[test]
    fn original_export_can_be_reconstructed_from_graph_and_message_metadata() -> Result<()> {
        let original: Value = serde_json::from_str(BRANCHES)?;
        let imported = parse_chatgpt(BRANCHES.as_bytes())?;
        let conversation = &imported[0];
        let mut reconstructed = conversation.metadata.clone();
        for message in &conversation.messages {
            reconstructed["mapping"][&message.source_id]["message"] = message.metadata.clone();
        }
        assert_eq!(reconstructed, original[0]);
        Ok(())
    }

    #[test]
    fn synthetic_demo_has_two_conversations_and_repeated_user_preferences() -> Result<()> {
        let imported = parse_chatgpt(include_bytes!("../tests/fixtures/chatgpt.json"))?;
        assert_eq!(imported.len(), 2);
        for conversation in imported {
            assert_eq!(conversation.metadata["synthetic_demo"], true);
            assert_eq!(conversation.messages.len(), 4);
            assert!(conversation.messages.iter().any(|message| {
                message.role == "user"
                    && message
                        .content
                        .to_lowercase()
                        .contains("keep the implementation simple")
                    && message.content.contains("Rust")
                    && message.content.contains("SQLite")
            }));
        }
        Ok(())
    }

    #[test]
    fn missing_and_null_optional_values_stay_unknown() -> Result<()> {
        let mut missing = record();
        missing.as_object_mut().unwrap().remove("id");
        missing["conversation_id"] = json!("fallback-id");
        missing["mapping"]["user"]["message"]["author"] = Value::Null;
        let absent = parse(json!([missing.clone()]))?.remove(0);
        assert_eq!(absent.source_id, "fallback-id");
        assert_eq!(absent.title, None);
        assert_eq!(absent.created_at, None);
        assert_eq!(absent.updated_at, None);
        assert_eq!(absent.messages[0].created_at, None);
        assert_eq!(absent.messages[0].role, "unknown");

        missing["id"] = Value::Null;
        missing["title"] = Value::Null;
        missing["create_time"] = Value::Null;
        missing["update_time"] = Value::Null;
        missing["mapping"]["user"]["message"]["create_time"] = Value::Null;
        let nulls = parse(json!([missing]))?.remove(0);
        assert_eq!(nulls.title, None);
        assert_eq!(nulls.created_at, None);
        assert_eq!(nulls.updated_at, None);
        assert_eq!(nulls.messages[0].created_at, None);
        Ok(())
    }

    #[test]
    fn supports_code_text_and_preserves_unknown_content() -> Result<()> {
        let mut value = record();
        value["mapping"]["user"]["message"]["content"] =
            json!({"content_type": "code", "text": "print(42)", "language": "python"});
        assert_eq!(
            parse(json!([value.clone()]))?[0].messages[0].content,
            "print(42)"
        );
        value["mapping"]["user"]["message"]["content"] =
            json!({"content_type": "audio_asset_pointer", "audio_asset_pointer": "audio-id"});
        let imported = parse(json!([value]))?;
        assert_eq!(imported[0].messages[0].content, "");
        assert_eq!(
            imported[0].messages[0].metadata["content"]["audio_asset_pointer"],
            "audio-id"
        );
        Ok(())
    }

    #[test]
    fn reads_extracted_json_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("conversations.json");
        std::fs::write(&path, BRANCHES)?;
        assert_eq!(read_chatgpt(&path)?.len(), 1);
        Ok(())
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) -> Result<()> {
        let mut writer = zip::ZipWriter::new(File::create(path)?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in entries {
            writer.start_file(*name, options)?;
            writer.write_all(content.as_bytes())?;
        }
        writer.finish()?;
        Ok(())
    }

    #[test]
    fn reads_zip_member_directly_without_extraction() -> Result<()> {
        let directory = tempfile::tempdir()?;
        // ZIP magic detection also works without a .zip extension.
        let path = directory.path().join("export.data");
        write_zip(
            &path,
            &[
                ("chatgpt-export/conversations.json", BRANCHES),
                ("other.txt", "ignored"),
            ],
        )?;
        assert_eq!(read_chatgpt(&path)?, parse_chatgpt(BRANCHES.as_bytes())?);
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }

    #[test]
    fn rejects_missing_or_ambiguous_zip_members() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("export.zip");
        write_zip(&path, &[("other.json", "[]")])?;
        assert!(format!("{:#}", read_chatgpt(&path).unwrap_err()).contains("found 0"));
        write_zip(
            &path,
            &[
                ("one/conversations.json", "[]"),
                ("two/conversations.json", "[]"),
            ],
        )?;
        assert!(format!("{:#}", read_chatgpt(&path).unwrap_err()).contains("found 2"));
        Ok(())
    }

    #[test]
    fn enforces_streaming_byte_limit() -> Result<()> {
        assert_eq!(read_bounded(Cursor::new(b"1234"), 4)?, b"1234");
        let error = read_bounded(Cursor::new(b"12345"), 4).unwrap_err();
        assert!(error.to_string().contains("4-byte"));
        Ok(())
    }

    #[test]
    fn rejects_invalid_ids_and_malformed_records_with_context() {
        for invalid_id in [Value::Null, json!(""), json!("  "), json!(123)] {
            let mut value = record();
            value["id"] = invalid_id;
            let error = format!("{:#}", parse(json!([value])).unwrap_err());
            assert!(error.contains("array index 0"), "{error}");
            assert!(error.contains("id"), "{error}");
        }
        assert!(parse_chatgpt(br#"{"conversations": []}"#).is_err());
        assert!(parse(json!([null])).is_err());
        assert!(parse(json!([{"id": "x", "mapping": []}])).is_err());

        let mut value = record();
        value["mapping"]["user"]["message"]["content"]["parts"] = json!("invalid");
        let error = format!("{:#}", parse(json!([value])).unwrap_err());
        assert!(error.contains("conversation-1"), "{error}");
        assert!(error.contains("user"), "{error}");
        assert!(error.contains("content.parts"), "{error}");
    }

    #[test]
    fn rejects_broken_parent_links_and_cycles() {
        let mut missing = record();
        missing["mapping"]["user"]["parent"] = json!("absent");
        let error = format!("{:#}", parse(json!([missing])).unwrap_err());
        assert!(error.contains("missing parent"), "{error}");

        let mut cycle = record();
        cycle["mapping"]["root"]["parent"] = json!("user");
        let error = format!("{:#}", parse(json!([cycle])).unwrap_err());
        assert!(error.contains("parent cycle"), "{error}");
    }

    #[test]
    fn accepts_identical_repeats_but_rejects_conflicting_ids() -> Result<()> {
        let first = record();
        assert_eq!(parse(json!([first.clone(), first.clone()]))?.len(), 2);
        let mut changed = first.clone();
        changed["title"] = json!("A conflicting version");
        let error = format!("{:#}", parse(json!([first, changed])).unwrap_err());
        assert!(
            error.contains("conflicting duplicate conversation ID"),
            "{error}"
        );
        Ok(())
    }
}
