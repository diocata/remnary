use std::io::{Cursor, Write};

use serde_json::json;
use zip::write::SimpleFileOptions;

use super::*;

const BRANCHES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/chatgpt-branches.json"
));

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
fn scrubbed_published_export_retains_every_graph_and_message_field() -> Result<()> {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/upstream-chatgpt-weather.json"
    ));
    let original: Value = serde_json::from_slice(bytes)?;
    let imported = parse_chatgpt(bytes)?;
    assert_eq!(imported.len(), 1);
    let conversation = &imported[0];
    assert_eq!(conversation.messages.len(), 11);
    assert_eq!(
        conversation.metadata["mapping"].as_object().unwrap().len(),
        12
    );
    assert!(conversation.messages.iter().any(|m| m.role == "tool"));
    let mut reconstructed = conversation.metadata.clone();
    for message in &conversation.messages {
        reconstructed["mapping"][&message.source_id]["message"] = message.metadata.clone();
    }
    assert_eq!(reconstructed, original[0]);
    Ok(())
}

#[test]
fn synthetic_demo_has_two_conversations_and_repeated_user_preferences() -> Result<()> {
    let imported = parse_chatgpt(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/chatgpt.json"
    )))?;
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
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
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
fn numbered_zip_shards_are_numeric_ordered_bounded_and_never_extracted() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("export.zip");
    let mut first = record();
    first["id"] = json!("first");
    let mut second = record();
    second["id"] = json!("second");
    let first = json!([first]).to_string();
    let second = json!([second]).to_string();
    write_zip(
        &path,
        &[
            ("export/conversations-10.json", &second),
            ("export/conversations-2.json", &first),
            ("ignored.json", "invalid"),
        ],
    )?;
    let records = read_chatgpt(&path)?;
    assert_eq!(
        records
            .iter()
            .map(|c| c.source_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
    assert!(
        read_zip_with_limit(File::open(&path)?, (first.len() + second.len() - 1) as u64).is_err()
    );
    Ok(())
}

#[test]
fn zip_shards_reject_ambiguity_and_conflicting_duplicates() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("export.zip");
    for entries in [
        vec![("conversations.json", "[]"), ("conversations-0.json", "[]")],
        vec![
            ("one/conversations-0.json", "[]"),
            ("two/conversations-1.json", "[]"),
        ],
        vec![
            ("conversations-01.json", "[]"),
            ("conversations-1.json", "[]"),
        ],
    ] {
        write_zip(&path, &entries)?;
        assert!(read_chatgpt(&path).is_err());
    }
    let first = json!([record()]).to_string();
    let mut changed = record();
    changed["title"] = json!("conflicting title");
    let changed = json!([changed]).to_string();
    write_zip(
        &path,
        &[
            ("conversations-0.json", &first),
            ("conversations-1.json", &changed),
        ],
    )?;
    assert!(format!("{:#}", read_chatgpt(&path).unwrap_err()).contains("conflicting duplicate"));
    write_zip(
        &path,
        &[
            ("conversations-0.json", &first),
            ("conversations-1.json", &first),
        ],
    )?;
    assert_eq!(read_chatgpt(&path)?.len(), 2);
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
