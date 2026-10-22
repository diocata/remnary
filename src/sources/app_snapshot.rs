//! Selected Codex-app history views. These are partial views, not account exports.
use crate::model::{Conversation, Message};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, Read},
    path::Path,
};
const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub fn read_snapshot(path: &Path) -> Result<Conversation> {
    let mut bytes = Vec::new();
    if path == Path::new("-") {
        // One JSON line allows a caller to pipe a selected tool response into
        // encryption without writing a plaintext export to disk.
        std::io::stdin()
            .lock()
            .take(MAX_BYTES + 1)
            .read_until(b'\n', &mut bytes)?;
    } else {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "snapshot must be a regular file, not a symlink"
        );
        fs::File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
    }
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "app snapshot exceeds 64 MiB limit"
    );
    parse_snapshot(&serde_json::from_slice(&bytes).context("invalid app snapshot JSON")?)
}

pub fn is_partial(metadata: &Value) -> bool {
    matches!(
        metadata
            .pointer("/remnary_collection/format")
            .or_else(|| metadata.pointer("/observr_collection/format"))
            .and_then(Value::as_str),
        Some("codex-app-view-v1" | "claude-native-view-v1")
    )
}

pub fn parse_snapshot(value: &Value) -> Result<Conversation> {
    ensure!(
        value.get("schemaVersion").and_then(Value::as_u64) == Some(1),
        "unsupported app snapshot schema"
    );
    let thread = value
        .get("thread")
        .context("snapshot is missing thread metadata")?;
    let provider = match text(thread, "kind")? {
        "chatgpt" => "chatgpt",
        _ => bail!(
            "only ChatGPT message views are supported here; use local JSONL for Codex attribution"
        ),
    };
    let id = text(thread, "id")?.to_owned();
    let order = value
        .pointer("/page/order")
        .and_then(Value::as_str)
        .context("snapshot is missing page order")?;
    ensure!(
        matches!(order, "newest_first" | "oldest_first"),
        "unsupported snapshot page order"
    );
    let mut turns = value
        .get("turns")
        .and_then(Value::as_array)
        .context("snapshot needs a turns array")?
        .iter()
        .collect::<Vec<_>>();
    if order == "newest_first" {
        turns.reverse();
    }
    let mut messages = Vec::new();
    let mut ids = BTreeSet::new();
    let mut turn_metadata = Vec::new();
    let mut other_items = Vec::new();
    for turn in turns {
        let mut metadata = turn
            .as_object()
            .context("snapshot turn must be an object")?
            .clone();
        metadata.remove("items");
        turn_metadata.push(Value::Object(metadata));
        for item in turn
            .get("items")
            .and_then(Value::as_array)
            .context("snapshot turn needs an items array")?
        {
            let kind = text(item, "type")?;
            let (mut role, content) = match kind {
                "userMessage" => {
                    let parts = item
                        .get("content")
                        .and_then(Value::as_array)
                        .context("user message needs content parts")?;
                    let content = parts
                        .iter()
                        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                        .map(|part| text(part, "text"))
                        .collect::<Result<Vec<_>>>()?
                        .join("\n");
                    ("user", content)
                }
                "agentMessage" => ("assistant", text(item, "text")?.to_owned()),
                _ => {
                    other_items.push(item.clone());
                    continue;
                }
            };
            if role == "user" && item.get("truncated").and_then(Value::as_bool) == Some(true) {
                role = "truncated_user";
            }
            let source_id = text(item, "id")?.to_owned();
            ensure!(
                !source_id.is_empty() && ids.insert(source_id.clone()),
                "duplicate or empty snapshot message ID"
            );
            messages.push(Message{source_id,parent_source_id:None,role:role.into(),content,created_at:None,
                metadata:json!({"app_item":item,"turn_id":turn.get("id"),"turn_started_at":turn.get("startedAt"),"turn_completed_at":turn.get("completedAt")})});
        }
    }
    ensure!(!id.is_empty(), "snapshot conversation ID is empty");
    let title = match thread.get("title") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => bail!("snapshot title must be text or null"),
    };
    Ok(Conversation {
        provider: provider.into(),
        source_id: id,
        title,
        created_at: time(thread, "createdAt")?,
        updated_at: time(thread, "updatedAt")?,
        messages,
        metadata: json!({"remnary_collection":{"format":"codex-app-view-v1","partial":true,"page":value.get("page"),"source_pages":value.get("source_pages")},
            "turn_metadata":turn_metadata,"other_items":other_items}),
    })
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("snapshot field {key} must be text"))
}
fn time(value: &Value, key: &str) -> Result<Option<f64>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => Ok(Some(n.as_f64().context("invalid snapshot timestamp")?)),
        _ => bail!("snapshot timestamp must be numeric or null"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        json!({"schemaVersion":1,"thread":{"id":"test","kind":"chatgpt","title":"Synthetic","createdAt":1000,"updatedAt":2000},"page":{"order":"newest_first","hasMore":true},"turns":[{"id":"second","startedAt":1800,"items":[{"type":"userMessage","id":"u2","content":[{"type":"text","text":"Keep code simple."}]},{"type":"agentMessage","id":"a2","text":"Truncated assistant view","truncated":true}]},{"id":"first","startedAt":1000,"items":[{"type":"userMessage","id":"u1","content":[{"type":"text","text":"I prefer Rust."}]}]}]})
    }
    #[test]
    fn preserves_actual_ids_partial_coverage_unknown_dates_and_truncation() -> Result<()> {
        let source = parse_snapshot(&fixture())?;
        assert_eq!(source.provider, "chatgpt");
        assert_eq!(source.source_id, "test");
        assert_eq!(source.messages[0].source_id, "u1");
        assert!(
            source
                .messages
                .iter()
                .all(|m| m.created_at.is_none() && m.parent_source_id.is_none())
        );
        assert_eq!(source.messages[2].metadata["app_item"]["truncated"], true);
        assert!(is_partial(&source.metadata));
        assert_eq!(
            crate::intelligence::extract_rules(&source, 1)
                .observations
                .len(),
            2
        );
        Ok(())
    }
    #[test]
    fn partial_views_cannot_replace_full_or_drop_previously_imported_messages() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = crate::store::Store::init(dir.path())?;
        let original = parse_snapshot(&fixture())?;
        store.import(std::slice::from_ref(&original))?;
        assert_eq!(store.import(std::slice::from_ref(&original))?.unchanged, 1);
        let mut shorter = original.clone();
        shorter.messages.pop();
        assert!(store.import(&[shorter]).is_err());
        let mut full = original.clone();
        full.metadata = json!({"complete_provider_export":true});
        store.import(&[full])?;
        assert!(store.import(&[original]).is_err());
        assert_eq!(store.stats()?.revisions, 2);
        Ok(())
    }

    #[test]
    fn legacy_collection_metadata_remains_partial() {
        for format in ["codex-app-view-v1", "claude-native-view-v1"] {
            assert!(is_partial(
                &json!({"observr_collection": {"format": format}})
            ));
        }
        assert!(!is_partial(
            &json!({"observr_collection": {"format": "unknown"}})
        ));
    }

    #[test]
    fn native_views_keep_partial_coverage_and_import_safeguards() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = crate::store::Store::init(dir.path())?;
        let mut source = parse_snapshot(&fixture())?;
        source.provider = "claude".into();
        source.metadata["remnary_collection"]["format"] = json!("claude-native-view-v1");
        assert!(is_partial(&source.metadata));
        assert!(
            crate::vault::snapshot_markdown(&source)?
                .1
                .contains("Coverage: partial app history view")
        );
        store.import(std::slice::from_ref(&source))?;
        let mut shorter = source.clone();
        shorter.messages.pop();
        assert!(store.import(&[shorter]).is_err());
        let mut full = source.clone();
        full.metadata = json!({"complete_provider_export": true});
        store.import(&[full])?;
        assert!(store.import(&[source]).is_err());
        Ok(())
    }

    #[test]
    fn truncated_user_views_cannot_supply_facts_and_exports_explain_coverage() -> Result<()> {
        let mut value = fixture();
        value["turns"][1]["items"][0]["truncated"] = json!(true);
        let source = parse_snapshot(&value)?;
        assert_eq!(source.messages[0].role, "truncated_user");
        let output = crate::intelligence::extract_rules(&source, 1);
        assert_eq!(output.observations.len(), 1);
        assert!(
            output
                .observations
                .iter()
                .all(|o| o.evidence.iter().all(|e| e.source_message_id != "u1"))
        );
        let (_, markdown) = crate::vault::snapshot_markdown(&source)?;
        assert!(markdown.contains("Coverage: partial app history view"));
        assert!(markdown.contains("Source text is truncated"));
        assert!(!markdown.contains("**Parent:** root"));
        Ok(())
    }
}
