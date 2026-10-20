//! Bounded user-message windows for local extraction. Raw evidence stays intact.
use crate::model::Conversation;
use anyhow::{Result, ensure};

const MAX_TEXT_BYTES: usize = 8192;
const MAX_BATCHES: usize = 256;
const OVERLAP_BYTES: usize = 256;

pub(crate) fn model_batches(source: &Conversation) -> Result<Vec<Conversation>> {
    let mut batches = Vec::new();
    let mut batch = source.clone();
    batch.messages.clear();
    batch.metadata = serde_json::json!({});
    if let Some(title) = &mut batch.title {
        title.truncate(boundary(title, title.len().min(512)));
    }
    let template = batch.clone();
    let mut bytes = 0;
    for message in source
        .messages
        .iter()
        .filter(|m| m.role.eq_ignore_ascii_case("user") && !m.content.trim().is_empty())
    {
        let mut start = 0;
        while start < message.content.len() {
            let end = boundary(
                &message.content,
                (start + MAX_TEXT_BYTES).min(message.content.len()),
            );
            let text = &message.content[start..end];
            if !batch.messages.is_empty() && bytes + text.len() > MAX_TEXT_BYTES {
                batches.push(batch);
                ensure!(
                    batches.len() < MAX_BATCHES,
                    "conversation exceeds the 256 local analysis window limit"
                );
                batch = template.clone();
                bytes = 0;
            }
            let mut excerpt = message.clone();
            excerpt.content = text.to_owned();
            excerpt.metadata = serde_json::json!({});
            bytes += text.len();
            batch.messages.push(excerpt);
            if end == message.content.len() {
                break;
            }
            // Flush each long-message fragment: source message identities may
            // repeat across windows, but never inside one data envelope.
            batches.push(batch);
            ensure!(
                batches.len() < MAX_BATCHES,
                "conversation exceeds the 256 local analysis window limit"
            );
            batch = template.clone();
            bytes = 0;
            start = boundary(&message.content, end - OVERLAP_BYTES);
        }
    }
    if !batch.messages.is_empty() {
        batches.push(batch);
    }
    Ok(batches)
}

fn boundary(text: &str, mut position: usize) -> usize {
    while !text.is_char_boundary(position) {
        position -= 1;
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Message;
    use serde_json::json;

    fn source(text: String) -> Conversation {
        Conversation {
            provider: "test".into(),
            source_id: "one".into(),
            title: Some("Title".into()),
            created_at: None,
            updated_at: None,
            metadata: json!({"private":"omitted"}),
            messages: vec![Message {
                source_id: "human".into(),
                parent_source_id: None,
                role: "user".into(),
                content: text,
                created_at: None,
                metadata: json!({"raw":"omitted"}),
            }],
        }
    }

    #[test]
    fn unicode_windows_preserve_every_byte_and_source_identity_with_overlap() -> Result<()> {
        let text = "Rust café 🦀. ".repeat(2000);
        let source = source(text.clone());
        let batches = model_batches(&source)?;
        assert!(batches.len() > 1);
        let mut reconstructed = String::new();
        for (index, batch) in batches.iter().enumerate() {
            assert_eq!(batch.stable_id(), source.stable_id());
            assert_eq!(batch.messages.len(), 1);
            let message = &batch.messages[0];
            assert_eq!(message.source_id, "human");
            assert!(message.content.len() <= MAX_TEXT_BYTES);
            assert!(text.contains(&message.content));
            assert_eq!(message.metadata, json!({}));
            if index == 0 {
                reconstructed.push_str(&message.content);
            } else {
                let previous = &batches[index - 1].messages[0].content;
                let start = boundary(previous, previous.len() - OVERLAP_BYTES);
                let overlap = previous.len() - start;
                assert_eq!(&previous[start..], &message.content[..overlap]);
                reconstructed.push_str(&message.content[overlap..]);
            }
        }
        assert_eq!(reconstructed, text);
        Ok(())
    }

    #[test]
    fn no_assistant_or_provider_context_enters_the_extraction_windows() -> Result<()> {
        let mut source = source("I prefer Rust.".into());
        let mut assistant = source.messages[0].clone();
        assistant.role = "assistant".into();
        source.messages.push(assistant);
        let batches = model_batches(&source)?;
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].messages.len(), 1);
        source.messages[0].role = "agent".into();
        assert!(model_batches(&source)?.is_empty());
        Ok(())
    }

    #[test]
    fn oversized_work_fails_before_partial_analysis_can_be_saved() {
        assert!(model_batches(&source("x".repeat(MAX_TEXT_BYTES * MAX_BATCHES))).is_err());
    }
}
