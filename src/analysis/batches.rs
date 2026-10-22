//! Bounded user-message windows for local extraction. Raw evidence stays intact.
use crate::model::Conversation;
use anyhow::{Result, ensure};

const MAX_TEXT_BYTES: usize = 8192;
const MAX_BATCHES: usize = 256;
const OVERLAP_BYTES: usize = 256;

pub(crate) fn model_batches(source: &Conversation, revision: i64) -> Result<Vec<Conversation>> {
    let mut batches = Vec::new();
    let template = Conversation {
        provider: source.provider.clone(),
        source_id: source.source_id.clone(),
        title: source
            .title
            .as_ref()
            .map(|title| title[..boundary(title, title.len().min(512))].to_owned()),
        created_at: source.created_at,
        updated_at: source.updated_at,
        metadata: serde_json::json!({}),
        messages: Vec::new(),
    };
    let mut batch = template.clone();
    let mut bytes = 0;
    for (message, span) in source.messages.iter().flat_map(|m| {
        crate::intelligence::personal_evidence_spans(m)
            .into_iter()
            .map(move |span| (m, span))
    }) {
        let mut start = 0;
        while start < span.len() {
            let mut end = boundary(span, (start + MAX_TEXT_BYTES).min(span.len()));
            if !batch.messages.is_empty()
                && (bytes + end - start > MAX_TEXT_BYTES
                    || batch
                        .messages
                        .iter()
                        .any(|m| m.source_id == message.source_id))
            {
                batches.push(batch);
                ensure!(
                    batches.len() < MAX_BATCHES,
                    "conversation exceeds the 256 local analysis window limit"
                );
                batch = template.clone();
                bytes = 0;
            }
            loop {
                batch.messages.push(crate::model::Message {
                    source_id: message.source_id.clone(),
                    parent_source_id: message.parent_source_id.clone(),
                    role: message.role.clone(),
                    content: span[start..end].to_owned(),
                    created_at: message.created_at,
                    metadata: serde_json::json!({}),
                });
                if crate::ollama::analysis_input_fits(&batch, revision)? {
                    break;
                }
                batch.messages.pop();
                if !batch.messages.is_empty() {
                    batches.push(batch);
                    ensure!(
                        batches.len() < MAX_BATCHES,
                        "conversation exceeds the 256 local analysis window limit"
                    );
                    batch = template.clone();
                    bytes = 0;
                } else {
                    let smaller = boundary(span, start + (end - start) / 2);
                    ensure!(
                        smaller > start && smaller < end,
                        "source identifiers cannot fit the bounded local analysis envelope"
                    );
                    end = smaller;
                }
            }
            bytes += end - start;
            if end == span.len() {
                break;
            }
            batches.push(batch);
            ensure!(
                batches.len() < MAX_BATCHES,
                "conversation exceeds the 256 local analysis window limit"
            );
            batch = template.clone();
            bytes = 0;
            let overlap = OVERLAP_BYTES.min((end - start) / 2);
            let next = boundary(span, end - overlap);
            start = if next > start { next } else { end };
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
    fn framing_budget_splits_many_short_messages_without_losing_ids() -> Result<()> {
        let mut source = source("Continue".into());
        source.messages = (0..200)
            .map(|index| Message {
                source_id: format!("message-{index:04}-{}", "x".repeat(64)),
                parent_source_id: Some(format!("parent-{index:04}-{}", "y".repeat(64))),
                role: "user".into(),
                content: "Continue".into(),
                created_at: None,
                metadata: json!({"private":"omit"}),
            })
            .collect();
        let batches = model_batches(&source, 1234567)?;
        assert!(batches.len() > 1);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.messages.len())
                .sum::<usize>(),
            200
        );
        let actual = batches
            .iter()
            .flat_map(|batch| &batch.messages)
            .map(|message| &message.source_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual.len(), 200);
        for batch in batches {
            assert!(crate::ollama::analysis_input_fits(&batch, 1234567)?);
        }
        Ok(())
    }

    #[test]
    fn escaped_text_is_split_by_serialized_budget_with_exact_source_fragments() -> Result<()> {
        let source = source("\u{0001}\"\\\n".repeat(5000));
        let batches = model_batches(&source, 1)?;
        assert!(batches.len() > 2);
        assert!(
            batches
                .iter()
                .any(|batch| batch.messages[0].content.len() < MAX_TEXT_BYTES)
        );
        for batch in batches {
            assert!(crate::ollama::analysis_input_fits(&batch, 1)?);
            assert!(
                source.messages[0]
                    .content
                    .contains(&batch.messages[0].content)
            );
        }
        Ok(())
    }

    #[test]
    fn separate_panel_answers_keep_exact_spans_without_duplicate_message_ids() -> Result<()> {
        let text = r#"<send_user_message_question_reply>[{"question":"Assistant-only assertion","answer":"I prefer Rust."},{"answer":"Keep code simple."}]</send_user_message_question_reply>"#;
        let original = source(text.into());
        let batches = model_batches(&original, 1)?;
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].messages[0].content, "I prefer Rust.");
        assert_eq!(batches[1].messages[0].content, "Keep code simple.");
        for batch in batches {
            assert!(text.contains(&batch.messages[0].content));
        }
        Ok(())
    }

    #[test]
    fn unicode_windows_preserve_every_byte_and_source_identity_with_overlap() -> Result<()> {
        let text = "Rust café 🦀. ".repeat(2000);
        let source = source(text.clone());
        let batches = model_batches(&source, 1)?;
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
        let batches = model_batches(&source, 1)?;
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].messages.len(), 1);
        source.messages[0].role = "agent".into();
        assert!(model_batches(&source, 1)?.is_empty());
        Ok(())
    }

    #[test]
    fn oversized_work_fails_before_partial_analysis_can_be_saved() {
        assert!(model_batches(&source("x".repeat(MAX_TEXT_BYTES * MAX_BATCHES)), 1).is_err());
    }
}
