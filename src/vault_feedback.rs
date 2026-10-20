//! Explicit import of user-authored review blocks outside generated Markdown.
use crate::{
    analysis_store::FeedbackAction,
    knowledge::{GENERATED_END, GENERATED_START, split_managed_section},
    store::Store,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultFeedback {
    pub observation_id: String,
    pub action: FeedbackAction,
    pub expected_feedback_count: usize,
    pub text: Option<String>,
}

/// One explicitly selected file/action per call. Generated content and nested
/// code fences are never interpreted as feedback. The input file is untouched.
pub fn read_feedback(path: &Path) -> Result<VaultFeedback> {
    let metadata = fs::symlink_metadata(path).context("inspect selected feedback note")?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "feedback note must be a regular file, not a symlink"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "feedback note exceeds 1 MiB limit"
    );
    let content = String::from_utf8(bytes).context("feedback note must be UTF-8")?;
    let sections = if content.contains(GENERATED_START) || content.contains(GENERATED_END) {
        let (before, _, after) = split_managed_section(&content)?;
        vec![before, after]
    } else {
        vec![content.as_str()]
    };
    let mut blocks = Vec::new();
    for section in sections {
        blocks.extend(feedback_blocks(section)?);
    }
    ensure!(
        blocks.len() == 1,
        "note must contain exactly one top-level observr-feedback code block outside generated sections"
    );
    let feedback: VaultFeedback = serde_json::from_str(&blocks[0])
        .map_err(|_| anyhow::anyhow!("feedback block does not match the review schema"))?;
    ensure!(
        feedback.observation_id.len() == 64
            && feedback
                .observation_id
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "feedback observation_id must be a 64-character identity from discover --json"
    );
    ensure!(
        matches!(feedback.action, FeedbackAction::Correct) == feedback.text.is_some(),
        "only correct feedback requires a text field"
    );
    ensure!(
        feedback
            .text
            .as_ref()
            .is_none_or(|s| !s.trim().is_empty() && s.len() <= 16_384),
        "correction text must be nonempty and at most 16 KiB"
    );
    Ok(feedback)
}

pub fn check_feedback(store: &Store, feedback: &VaultFeedback) -> Result<()> {
    let record = store
        .observations(true)?
        .into_iter()
        .find(|r| r.id == feedback.observation_id)
        .context("observation has no currently available evidence; review discover --all first")?;
    ensure!(
        record.feedback_count == feedback.expected_feedback_count,
        "feedback conflict: note expected {} prior reviews, but archive has {}; review current knowledge before retrying",
        feedback.expected_feedback_count,
        record.feedback_count
    );
    Ok(())
}

pub fn apply_feedback(store: &mut Store, feedback: &VaultFeedback) -> Result<()> {
    check_feedback(store, feedback)?;
    store.feedback_checked(
        &feedback.observation_id,
        feedback.action,
        feedback.text.as_deref(),
        feedback.expected_feedback_count,
    )
}

fn feedback_blocks(content: &str) -> Result<Vec<String>> {
    let mut fence: Option<(u8, usize, bool)> = None;
    let mut current = String::new();
    let mut blocks = Vec::new();
    for raw in content.lines() {
        let line = raw.trim_start_matches(' ');
        let indent = raw.len() - line.len();
        if let Some((marker, count, capture)) = fence {
            let closing = line.bytes().take_while(|b| *b == marker).count();
            if indent <= 3 && closing >= count && line[closing..].trim().is_empty() {
                if capture {
                    blocks.push(std::mem::take(&mut current));
                }
                fence = None;
            } else if capture {
                current.push_str(raw);
                current.push('\n');
            }
            continue;
        }
        if indent > 3 {
            continue;
        }
        let Some(marker @ (b'`' | b'~')) = line.bytes().next() else {
            continue;
        };
        let count = line.bytes().take_while(|b| *b == marker).count();
        if count < 3 {
            continue;
        }
        let info = line[count..].trim();
        fence = Some((marker, count, info == "observr-feedback"));
    }
    ensure!(
        fence.is_none(),
        "Markdown code block outside generated sections is not closed"
    );
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::{AnalysisEngine, analyze},
        import::read_chatgpt,
        knowledge::sync_vault,
    };
    use serde_json::json;

    #[test]
    fn explicit_markdown_correction_preserves_note_and_rejects_stale_replay() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut store = Store::init(&temp.path().join("data"))?;
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json");
        store.import(&read_chatgpt(&fixture)?)?;
        analyze(&mut store, AnalysisEngine::Rules)?;
        let record = store.observations(false)?.remove(0);
        let vault = temp.path().join("vault");
        sync_vault(&mut store, &vault, "UTC")?;
        let path = vault.join("Profile.md");
        let mut content = fs::read_to_string(&path)?;
        content.push_str(&format!("\nMy review:\n```observr-feedback\n{}\n```\n",json!({"observation_id":record.id,"action":"correct","expected_feedback_count":0,"text":"Simple implementations for small projects"})));
        fs::write(&path, &content)?;
        let feedback = read_feedback(&path)?;
        check_feedback(&store, &feedback)?;
        assert_eq!(store.observations(true)?[0].feedback_count, 0);
        apply_feedback(&mut store, &feedback)?;
        assert!(apply_feedback(&mut store, &feedback).is_err());
        let record = store.observations(false)?.remove(0);
        assert_eq!(record.feedback_count, 1);
        assert_eq!(
            record.display_text(),
            "Simple implementations for small projects"
        );
        // Projection cleanup preserves all user-authored review bytes.
        assert!(fs::read_to_string(&path)?.ends_with(content.split_once("My review:").unwrap().1));
        sync_vault(&mut store, &vault, "UTC")?;
        assert!(fs::read_to_string(&path)?.contains("My review:"));
        assert!(check_feedback(&store, &read_feedback(&path)?).is_err());
        Ok(())
    }

    #[test]
    fn code_examples_generated_blocks_and_multiple_actions_are_never_applied() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("review.md");
        let block = format!(
            "```observr-feedback\n{}\n```\n",
            json!({"observation_id":"a".repeat(64),"action":"confirm","expected_feedback_count":0})
        );
        for content in [
            format!("{GENERATED_START}{block}{GENERATED_END}"),
            format!("````markdown\n{block}````\n"),
            format!("{block}{block}"),
            format!("> {block}"),
            "```observr-feedback\n{}".into(),
        ] {
            fs::write(&path, content)?;
            assert!(read_feedback(&path).is_err());
        }
        fs::write(&path, &block)?;
        assert!(read_feedback(&path).is_ok());
        #[cfg(unix)]
        {
            let link = temp.path().join("link.md");
            std::os::unix::fs::symlink(&path, &link)?;
            assert!(read_feedback(&link).is_err());
        }
        Ok(())
    }
}
