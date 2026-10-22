//! Related discussions are a view over distinct claims, never a merged preference.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::Serialize;

use crate::{
    analysis_store::ObservationRecord, intelligence::EvidenceRef, model::digest,
    origins::OriginIndex, store::Store,
};

#[derive(Debug, Clone, Serialize)]
pub struct TopicThread {
    pub id: String,
    pub subject: String,
    pub topic: String,
    pub distinct_conversations: usize,
    /// Distinct cited source messages, after copied-history correlation.
    pub occurrences: usize,
    pub providers: Vec<String>,
    pub observation_ids: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
}

fn key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

impl Store {
    /// Group exact normalized topic labels while keeping every claim/context
    /// and review identity separate. A related topic is not an enduring trait.
    pub fn topic_threads(&self) -> Result<Vec<TopicThread>> {
        self.topic_threads_from(&self.observations(false)?)
    }

    pub(crate) fn topic_threads_from(
        &self,
        records: &[ObservationRecord],
    ) -> Result<Vec<TopicThread>> {
        let origins = OriginIndex::load(&self.connection)?;
        Ok(group_topics(records, &origins))
    }
}

fn group_topics(records: &[ObservationRecord], origins: &OriginIndex) -> Vec<TopicThread> {
    let mut groups = BTreeMap::<(String, String), Vec<&ObservationRecord>>::new();
    for record in records {
        if !record.status.is_visible()
            // A free-text correction need not retain the model's topic label.
            || record.user_value.is_some()
        {
            continue;
        }
        let subject = key(&record.observation.subject);
        let topic = key(&record.observation.topic);
        if !subject.is_empty() && !topic.is_empty() {
            groups.entry((subject, topic)).or_default().push(record);
        }
    }
    let mut threads = Vec::new();
    for ((subject, topic), members) in groups {
        let evidence = members
            .iter()
            .flat_map(|r| r.observation.evidence.iter().cloned())
            .collect::<BTreeSet<_>>();
        let conversations = evidence
            .iter()
            .map(|e| origins.conversation(&e.conversation_id))
            .collect::<BTreeSet<_>>();
        if conversations.len() < 2 {
            continue;
        }
        let occurrences = evidence
            .iter()
            .map(|e| {
                (
                    origins.conversation(&e.conversation_id),
                    origins.message(&e.conversation_id, &e.source_message_id),
                )
            })
            .collect::<BTreeSet<_>>()
            .len();
        let providers = evidence
            .iter()
            .map(|e| e.provider.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let observation_ids = members
            .iter()
            .map(|r| r.id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        threads.push(TopicThread {
            id: digest(
                &serde_json::to_vec(&("topic-thread-v1", &subject, &topic))
                    .expect("string tuple serializes"),
            ),
            subject,
            topic,
            distinct_conversations: conversations.len(),
            occurrences,
            providers,
            observation_ids,
            evidence: evidence.into_iter().collect(),
        });
    }
    threads.sort_by(|a, b| {
        b.distinct_conversations
            .cmp(&a.distinct_conversations)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.id.cmp(&b.id))
    });
    threads
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis_store::{FeedbackAction, ObservationStatus, Processor},
        intelligence::{
            AnalysisOutput, CandidateObservation, ExtractionKind, InferenceType, extract_rules,
        },
        model::{Conversation, Message},
    };
    use serde_json::json;

    fn seed(store: &mut Store) -> Result<Vec<ObservationRecord>> {
        let sources = [
            ("one", "I compare Rust and Go for web APIs.", "for web APIs"),
            (
                "two",
                "I compare Go and Python for embedded projects.",
                "for embedded projects",
            ),
        ]
        .into_iter()
        .map(|(id, content, _)| Conversation {
            provider: "synthetic".into(),
            source_id: id.into(),
            title: None,
            created_at: None,
            updated_at: None,
            metadata: json!({}),
            messages: vec![Message {
                source_id: "u1".into(),
                parent_source_id: None,
                role: "user".into(),
                content: content.into(),
                created_at: None,
                metadata: json!({}),
            }],
        })
        .collect::<Vec<_>>();
        store.import(&sources)?;
        let processor = Processor::rules();
        for target in store.analysis_targets(&processor)? {
            let source = &target.conversation;
            let content = &source.messages[0].content;
            let context = if source.source_id == "one" {
                "for web APIs"
            } else {
                "for embedded projects"
            };
            let observation = CandidateObservation {
                kind: ExtractionKind::Topic,
                inference: InferenceType::Explicit,
                subject: "user".into(),
                topic: if source.source_id == "one" {
                    "Tool   Research"
                } else {
                    "tool research"
                }
                .into(),
                canonical_value: content.clone(),
                context: Some(context.into()),
                summary: content.clone(),
                decision_event: None,
                evidence: vec![EvidenceRef {
                    provider: source.provider.clone(),
                    conversation_id: source.stable_id(),
                    revision: target.revision,
                    source_message_id: "u1".into(),
                    quote: content.clone(),
                }],
            };
            store.save_analysis(
                &target,
                &processor,
                AnalysisOutput {
                    observations: vec![observation],
                    ..Default::default()
                },
            )?;
        }
        store.observations(false)
    }

    #[test]
    fn related_topics_preserve_distinct_claims_contexts_and_message_counts() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        let original = seed(&mut store)?;
        let mut records = original.clone();
        let mut duplicate = records[0].clone();
        duplicate.id = "another-claim-citing-the-same-message".into();
        records.push(duplicate);
        let threads = store.topic_threads_from(&records)?;
        assert_eq!(threads.len(), 1);
        let thread = &threads[0];
        assert_eq!(thread.topic, "tool research");
        assert_eq!(thread.distinct_conversations, 2);
        assert_eq!(thread.occurrences, 2);
        assert_eq!(thread.evidence.len(), 2);
        assert_eq!(thread.observation_ids.len(), 3);
        assert!(store.topic_threads_from(&records[..1])?.is_empty());
        let unchanged = store.observations(false)?;
        assert_eq!(
            serde_json::to_value(&unchanged)?,
            serde_json::to_value(&original)?
        );
        assert!(
            unchanged
                .iter()
                .all(|r| r.status == ObservationStatus::Candidate)
        );
        assert_eq!(
            unchanged
                .iter()
                .filter_map(|r| r.observation.context.as_deref())
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        Ok(())
    }

    #[test]
    fn review_exclusion_and_deletion_remove_topic_support_without_stale_labels() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        let records = seed(&mut store)?;
        let id = &records[0].id;
        assert_eq!(store.topic_threads()?.len(), 1);
        store.feedback(id, FeedbackAction::Dismiss, None)?;
        assert!(store.topic_threads()?.is_empty());
        store.feedback(id, FeedbackAction::Reset, None)?;
        store.feedback(
            id,
            FeedbackAction::Correct,
            Some("A different subject altogether"),
        )?;
        assert!(store.topic_threads()?.is_empty());
        store.feedback(id, FeedbackAction::Reset, None)?;
        let source = &records[0].observation.evidence[0].conversation_id;
        store.exclude_conversation(source, true)?;
        assert!(store.topic_threads()?.is_empty());
        store.exclude_conversation(source, false)?;
        assert_eq!(store.topic_threads()?.len(), 1);
        store.delete_observation(id)?;
        assert!(store.topic_threads()?.is_empty());
        Ok(())
    }

    #[test]
    fn copied_kiro_execution_files_do_not_establish_topic_recurrence() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(&dir.path().join("archive"))?;
        let reply = "A substantive explanation of the requested workflow. ".repeat(12);
        for (id, chat) in [
            (
                "one",
                json!([{"role":"human","content":"I prefer Rust."},{"role":"bot","content":reply}]),
            ),
            (
                "two",
                json!([{"role":"human","content":"I prefer Rust."},{"role":"bot","content":reply},{"role":"human","content":"I prefer SQLite."}]),
            ),
        ] {
            let path = dir.path().join(format!("{id}.chat"));
            std::fs::write(
                &path,
                serde_json::to_vec(&json!({"executionId":id,"chat":chat}))?,
            )?;
            store.import(&crate::local_sources::read_source(
                &path,
                crate::local_sources::SourceFormat::Kiro,
            )?)?;
        }
        let processor = Processor::rules();
        for target in store.analysis_targets(&processor)? {
            store.save_analysis(
                &target,
                &processor,
                extract_rules(&target.conversation, target.revision),
            )?;
        }
        let records = store.observations(false)?;
        assert!(records.len() >= 2);
        assert!(!group_topics(&records, &OriginIndex::default()).is_empty());
        assert!(store.topic_threads()?.is_empty());
        Ok(())
    }

    #[test]
    fn vault_topic_notes_link_both_claims_and_preserve_owner_annotations() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(&dir.path().join("archive"))?;
        let records = seed(&mut store)?;
        let topic = store.topic_threads()?.remove(0);
        let vault = dir.path().join("vault");
        crate::knowledge::sync_vault(&mut store, &vault, "UTC")?;
        let note = vault.join(format!("Topics/{}.md", topic.id));
        let body = std::fs::read_to_string(&note)?;
        assert!(body.contains("for web APIs"));
        assert!(body.contains("for embedded projects"));
        assert!(body.contains("../Transcripts/"));
        let links = regex::Regex::new(r"\[Source message\]\(([^)]+)#(message-[a-f0-9]+)\)")?;
        let mut checked = 0;
        for link in links.captures_iter(&body) {
            let source = note.parent().unwrap().join(&link[1]);
            assert!(std::fs::read_to_string(source)?.contains(&format!("id=\"{}\"", &link[2])));
            checked += 1;
        }
        assert_eq!(checked, 2);
        std::fs::write(&note, format!("{body}\nOwner annotation stays here.\n"))?;
        crate::knowledge::sync_vault(&mut store, &vault, "UTC")?;
        assert!(std::fs::read_to_string(&note)?.ends_with("Owner annotation stays here.\n"));
        store.feedback(&records[0].id, FeedbackAction::Reject, None)?;
        crate::knowledge::sync_vault(&mut store, &vault, "UTC")?;
        let remaining = std::fs::read_to_string(&note)?;
        assert!(remaining.contains("Owner annotation stays here."));
        assert!(!remaining.contains("for embedded projects"));
        assert!(!remaining.contains("for web APIs"));
        Ok(())
    }
}
