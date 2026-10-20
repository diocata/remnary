//! Human-readable, immutable Markdown snapshots. Markdown edits are never ingested.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::model::{Conversation, Message, digest};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub written: usize,
    pub unchanged: usize,
    pub conflicts: usize,
}

/// Return the exact immutable transcript filename and bytes used by export.
pub fn snapshot_markdown(conversation: &Conversation) -> Result<(String, String)> {
    let stable_id = conversation.stable_id();
    let fingerprint = conversation.fingerprint()?;
    Ok((
        format!("{stable_id}-{fingerprint}.md"),
        render(conversation, &stable_id, &fingerprint),
    ))
}

/// Export content-addressed snapshots into an explicitly selected directory.
///
/// Both parts of each filename are SHA-256 hashes; provider IDs never become
/// paths. Existing files, including manually edited notes, are never rewritten.
/// A changed conversation produces a new snapshot. An interrupted write can
/// leave a partial note, which a later export safely reports as a conflict.
pub fn export_markdown(conversations: &[Conversation], output: &Path) -> Result<ExportSummary> {
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(output)
        .with_context(|| format!("create export directory {}", output.display()))?;

    let mut summary = ExportSummary::default();
    for conversation in conversations {
        let stable_id = conversation.stable_id();
        let fingerprint = conversation.fingerprint()?;
        let path = output.join(format!("{stable_id}-{fingerprint}.md"));
        let content = render(conversation, &stable_id, &fingerprint);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(content.as_bytes())
                    .with_context(|| format!("write Markdown snapshot {}", path.display()))?;
                summary.written += 1;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                // Do not follow a preexisting symlink or try to read a directory
                // or special file. Those paths also count as preserved conflicts.
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("inspect existing snapshot {}", path.display()))?;
                if metadata.file_type().is_file()
                    && fs::read(&path)
                        .with_context(|| format!("read existing snapshot {}", path.display()))?
                        == content.as_bytes()
                {
                    summary.unchanged += 1;
                } else {
                    summary.conflicts += 1;
                }
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create Markdown snapshot {}", path.display()));
            }
        }
    }
    Ok(summary)
}

fn render(conversation: &Conversation, stable_id: &str, fingerprint: &str) -> String {
    let mut note = String::from("# Conversation snapshot\n\n");
    field(
        &mut note,
        "Title",
        conversation.title.as_deref().unwrap_or("Untitled"),
    );
    field(&mut note, "Provider", &conversation.provider);
    field(&mut note, "Conversation source ID", &conversation.source_id);
    note.push_str(&format!(
        "**Stable ID:** `{stable_id}`\n\n**Revision:** `{fingerprint}`\n\n"
    ));
    timestamp(&mut note, "Created (Unix seconds)", conversation.created_at);
    timestamp(&mut note, "Updated (Unix seconds)", conversation.updated_at);
    note.push_str(&format!(
        "**Messages:** {}\n\n",
        conversation.messages.len()
    ));
    note.push_str("This immutable snapshot contains available message text. Original source metadata is retained in the database. Changes to this note are preserved and are not imported.\n\n");

    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    let source_ids: HashSet<&str> = conversation
        .messages
        .iter()
        .map(|message| message.source_id.as_str())
        .collect();
    for message in &conversation.messages {
        if let Some(parent) = message.parent_source_id.as_deref() {
            children.entry(parent).or_default().push(&message.source_id);
        }
    }
    let branch_points = children
        .values()
        .filter(|children| children.len() > 1)
        .count();
    note.push_str(&format!("**Branch points:** {branch_points}\n\n"));
    if let Some(current_node) = conversation
        .metadata
        .get("current_node")
        .and_then(Value::as_str)
    {
        field(&mut note, "Source current branch tip", current_node);
    }
    note.push_str("## Transcript\n\n");

    for (index, message) in conversation.messages.iter().enumerate() {
        note.push_str(&format!(
            "<a id=\"{}\"></a>\n\n### Message {}\n\n",
            anchor(&message.source_id),
            index + 1
        ));
        field(&mut note, "Role", &message.role);
        field(&mut note, "Message source ID", &message.source_id);
        if let Some(parent) = message.parent_source_id.as_deref() {
            field(&mut note, "Parent source ID", parent);
            if source_ids.contains(parent) {
                note.push_str(&format!("[Go to parent message](#{})\n\n", anchor(parent)));
            } else {
                note.push_str("Parent message is absent from this snapshot.\n\n");
            }
        } else {
            note.push_str("**Parent:** root\n\n");
        }
        if let Some(descendants) = children.get(message.source_id.as_str()) {
            note.push_str(&format!(
                "**Child messages:** {}{}\n\n",
                descendants.len(),
                if descendants.len() > 1 {
                    " (branch point)"
                } else {
                    ""
                }
            ));
            for (child_index, child) in descendants.iter().enumerate() {
                note.push_str(&format!(
                    "[Child {}](#{})\n\n",
                    child_index + 1,
                    anchor(child)
                ));
            }
        }
        timestamp(&mut note, "Created (Unix seconds)", message.created_at);
        if has_unsupported_content(message) {
            note.push_str("**Non-text content:** This source message includes content that is not rendered here. Its original metadata is retained in the database.\n\n");
        }
        note.push_str("**Text:**\n\n");
        quoted(&mut note, &message.content);
    }
    note
}

pub fn message_anchor(source_id: &str) -> String {
    format!("message-{}", digest(source_id.as_bytes()))
}

fn anchor(source_id: &str) -> String {
    message_anchor(source_id)
}

fn field(note: &mut String, label: &str, value: &str) {
    note.push_str(&format!("**{label}:**\n\n"));
    quoted(note, value);
}

fn timestamp(note: &mut String, label: &str, value: Option<f64>) {
    match value {
        Some(value) => note.push_str(&format!("**{label}:** {value}\n\n")),
        None => note.push_str(&format!("**{label}:** unknown\n\n")),
    }
}

pub(crate) fn quoted(note: &mut String, value: &str) {
    // Keep source text literal: Markdown images, links, HTML, and wiki links
    // must not become active content. A fence longer than every source run
    // prevents source code fences from closing this generated code block.
    let longest_run = value
        .as_bytes()
        .split(|byte| *byte != b'`')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat((longest_run + 1).max(3));
    note.push_str(&format!("{fence}text\n"));
    note.push_str(value);
    if !value.ends_with(['\n', '\r']) {
        note.push('\n');
    }
    note.push_str(&format!("{fence}\n\n"));
}

fn has_unsupported_content(message: &Message) -> bool {
    let content = message.metadata.get("content").or_else(|| {
        message
            .metadata
            .get("message")
            .and_then(|message| message.get("content"))
    });
    let Some(content) = content else { return false };
    content
        .get("content_type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind != "text" && kind != "multimodal_text")
        || content
            .get("parts")
            .and_then(Value::as_array)
            .is_some_and(|parts| parts.iter().any(|part| !part.is_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn conversation() -> Conversation {
        Conversation {
            provider: "chatgpt".into(),
            source_id: "conversation-a".into(),
            title: Some("Example".into()),
            created_at: None,
            updated_at: None,
            metadata: json!({"current_node": "child-b"}),
            messages: vec![
                message("root", None),
                message("child-a", Some("root")),
                message("child-b", Some("root")),
            ],
        }
    }

    fn message(source_id: &str, parent: Option<&str>) -> Message {
        Message {
            source_id: source_id.into(),
            parent_source_id: parent.map(str::to_owned),
            role: "user".into(),
            content: "# Source heading\n<script>hello & goodbye</script>".into(),
            created_at: None,
            metadata: json!({"content": {"content_type": "text", "parts": ["text"]}}),
        }
    }

    #[test]
    fn exports_are_idempotent_and_keep_private_permissions() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let output = temp.path().join("notes");
        let conversations = [conversation()];
        assert_eq!(
            export_markdown(&conversations, &output)?,
            ExportSummary {
                written: 1,
                unchanged: 0,
                conflicts: 0
            }
        );
        assert_eq!(
            export_markdown(&conversations, &output)?,
            ExportSummary {
                written: 0,
                unchanged: 1,
                conflicts: 0
            }
        );
        let path = fs::read_dir(&output)?.next().unwrap()?.path();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&output)?.permissions().mode() & 0o777, 0o700);
            assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
        }
        Ok(())
    }

    #[test]
    fn preserves_edited_notes_and_writes_new_revisions() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut conversation = conversation();
        export_markdown(std::slice::from_ref(&conversation), temp.path())?;
        let path = fs::read_dir(temp.path())?.next().unwrap()?.path();
        fs::write(&path, "My personal annotations\n")?;
        assert_eq!(
            export_markdown(std::slice::from_ref(&conversation), temp.path())?,
            ExportSummary {
                written: 0,
                unchanged: 0,
                conflicts: 1
            }
        );
        assert_eq!(fs::read_to_string(&path)?, "My personal annotations\n");
        conversation.messages[0]
            .content
            .push_str("\nNew source revision");
        assert_eq!(export_markdown(&[conversation], temp.path())?.written, 1);
        assert_eq!(fs::read_to_string(path)?, "My personal annotations\n");
        assert_eq!(fs::read_dir(temp.path())?.count(), 2);
        Ok(())
    }

    #[test]
    fn hashes_path_ids_and_preserves_branch_evidence_and_non_text_notice() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let output = temp.path().join("notes");
        let mut conversation = conversation();
        conversation.provider = "../../outside".into();
        conversation.source_id = "../../../escape.md".into();
        conversation.messages[0].source_id = "../root</a>".into();
        conversation.messages[0].content = "# Source heading\n<script>hello & goodbye</script>\n![remote image](https://example.com/tracker.png)\n[[local link]]\n```\n````\n".into();
        conversation.messages[1].parent_source_id = Some("../root</a>".into());
        conversation.messages[2].parent_source_id = Some("../root</a>".into());
        conversation.messages[2].metadata = json!({"content": {"content_type": "multimodal_text", "parts": [{"asset_pointer": "image"}]}});
        export_markdown(&[conversation], &output)?;
        let entry = fs::read_dir(&output)?.next().unwrap()?;
        let name = entry.file_name().into_string().unwrap();
        let parts: Vec<_> = name.strip_suffix(".md").unwrap().split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(
            parts.iter().all(|part| part.len() == 64
                && part.chars().all(|character| character.is_ascii_hexdigit()))
        );
        let text = fs::read_to_string(entry.path())?;
        assert!(text.contains("**Branch points:** 1"));
        assert!(text.contains("**Child messages:** 2 (branch point)"));
        assert!(text.contains("**Parent source ID:**\n\n```text\n../root</a>\n```"));
        assert!(text.contains(&format!(
            "[Go to parent message](#{})",
            anchor("../root</a>")
        )));
        assert!(text.contains("`````text\n# Source heading\n<script>hello & goodbye</script>\n![remote image](https://example.com/tracker.png)\n[[local link]]\n```\n````\n`````\n"));
        assert!(text.contains("**Non-text content:**"));
        assert!(!temp.path().join("escape.md").exists());
        Ok(())
    }
}
