//! Conservative counting for Kiro's cumulative execution files. Keep every
//! source/evidence link; a copied complete history is not another utterance.
use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::Connection;

use crate::model::digest;

#[derive(Default)]
pub(crate) struct OriginIndex(BTreeMap<String, String>);
struct History {
    id: String,
    prefixes: Vec<String>,
    hash: String,
    has_user: bool,
    reply_bytes: usize,
}
impl OriginIndex {
    pub(crate) fn load(connection: &Connection) -> Result<Self> {
        // Native fallback IDs hash the complete original Kiro message plus its
        // occurrence number. System prompts alone do not establish a lineage.
        let mut query=connection.prepare(
            "SELECT c.id,m.source_id,m.role,length(CAST(m.content AS BLOB)) FROM conversations c
             JOIN conversation_revisions r ON r.conversation_id=c.id AND r.revision=c.current_revision
             JOIN messages m ON m.conversation_id=c.id AND m.revision=c.current_revision
             WHERE c.provider='kiro' AND json_extract(r.metadata,'$.source_format')='kiro-ide-chat-v1'
             AND lower(m.role)!='system' AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id=c.id)
             ORDER BY c.id,m.position")?;
        let mut rows = query.query([])?;
        let mut histories = Vec::<History>::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            if histories.last().is_none_or(|h| h.id != id) {
                histories.push(History {
                    id,
                    prefixes: vec![],
                    hash: digest(b"observr-kiro-prefix-v1"),
                    has_user: false,
                    reply_bytes: 0,
                });
            }
            let history = histories.last_mut().expect("current source was added");
            let message: String = row.get(1)?;
            let role: String = row.get(2)?;
            history.has_user |= role.eq_ignore_ascii_case("user");
            if history.has_user && role.eq_ignore_ascii_case("assistant") {
                history.reply_bytes = history
                    .reply_bytes
                    .saturating_add(usize::try_from(row.get::<_, i64>(3)?)?);
            }
            history.hash = digest(&serde_json::to_vec(&(&history.hash, &message, &role))?);
            history.prefixes.push(history.hash.clone());
        }
        // Require substantive reply history, not repeated greetings, system
        // boilerplate or identical user prompts with no copied answer.
        let mut endings = BTreeMap::<String, Vec<usize>>::new();
        for (index, history) in histories.iter().enumerate() {
            if history.has_user && history.reply_bytes >= 256 {
                endings.entry(history.hash.clone()).or_default().push(index);
            }
        }
        let mut parents = (0..histories.len()).collect::<Vec<_>>();
        for (index, history) in histories.iter().enumerate() {
            for prefix in &history.prefixes {
                if let Some(copies) = endings.get(prefix) {
                    for &other in copies {
                        let a = root(&mut parents, index);
                        let b = root(&mut parents, other);
                        parents[a.max(b)] = a.min(b);
                    }
                }
            }
        }
        let mut origins = BTreeMap::new();
        for (index, history) in histories.iter().enumerate() {
            origins.insert(
                history.id.clone(),
                histories[root(&mut parents, index)].id.clone(),
            );
        }
        Ok(Self(origins))
    }
    pub(crate) fn conversation(&self, id: &str) -> String {
        self.0.get(id).map(String::as_str).unwrap_or(id).to_owned()
    }
}
fn root(parents: &mut [usize], mut index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    while index != root {
        let next = parents[index];
        parents[index] = root;
        index = next;
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis_store::{ObservationStatus, Processor},
        intelligence::extract_rules,
        local_sources::{SourceFormat, read_source},
        model::Conversation,
        store::Store,
    };
    use serde_json::json;
    fn session(
        folder: &std::path::Path,
        id: &str,
        chat: serde_json::Value,
    ) -> Result<Conversation> {
        let path = folder.join(format!("{id}.chat"));
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({"executionId":id,"chat":chat}))?,
        )?;
        Ok(read_source(&path, SourceFormat::Kiro)?.remove(0))
    }
    fn analyze(store: &mut Store) -> Result<()> {
        let p = Processor::rules();
        for target in store.analysis_targets(&p)? {
            store.save_analysis(
                &target,
                &p,
                extract_rules(&target.conversation, target.revision),
            )?;
        }
        Ok(())
    }
    #[test]
    fn cumulative_executions_preserve_sources_without_inflating_recurrence() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut store = Store::init(&temp.path().join("archive"))?;
        let reply = "A substantive explanation of the requested workflow. ".repeat(12);
        let short = session(
            temp.path(),
            "one",
            json!([
            {"role":"human","content":"I prefer Rust."},{"role":"bot","content":reply}]),
        )?;
        let long = session(
            temp.path(),
            "two",
            json!([
            {"role":"human","content":"I prefer Rust."},{"role":"bot","content":reply},
            {"role":"human","content":"I prefer SQLite."}]),
        )?;
        store.import(&[short.clone(), long.clone()])?;
        analyze(&mut store)?;
        let records = store.observations(false)?;
        let rust = records
            .iter()
            .find(|r| r.observation.canonical_value == "prefer rust")
            .unwrap();
        assert_eq!(store.stats()?.conversations, 2);
        assert_eq!(rust.observation.evidence.len(), 2);
        assert_eq!(rust.distinct_conversations, 1);
        assert_eq!(rust.occurrences, 1);
        assert_eq!(rust.status, ObservationStatus::Candidate);
        assert!(!rust.conversation_origins.is_empty());
        let roundtrip: Vec<crate::analysis_store::ObservationRecord> =
            serde_json::from_value(serde_json::to_value(&records)?)?;
        let ranked = crate::reports::rank_discoveries(roundtrip, usize::MAX);
        let ranked_rust = ranked.iter().find(|r| r.id == rust.id).unwrap();
        assert_eq!(ranked_rust.distinct_conversations, 1);
        assert_eq!(ranked_rust.occurrences, 1);
        assert_eq!(store.snapshot(&short.stable_id(), None)?, short);
        assert_eq!(store.snapshot(&long.stable_id(), None)?, long);
        // A separate reply establishes another history despite the repeated prompt.
        let independent = session(
            temp.path(),
            "three",
            json!([
            {"role":"human","content":"I prefer Rust."},{"role":"bot","content":"A different substantive response for another independent conversation. ".repeat(10)}]),
        )?;
        store.import(std::slice::from_ref(&independent))?;
        analyze(&mut store)?;
        let records = store.observations(false)?;
        let rust = records
            .iter()
            .find(|r| r.observation.canonical_value == "prefer rust")
            .unwrap();
        assert_eq!(rust.distinct_conversations, 2);
        assert_eq!(rust.occurrences, 2);
        assert_eq!(rust.status, ObservationStatus::Supported);
        store.exclude_conversation(&independent.stable_id(), true)?;
        let records = store.observations(false)?;
        let rust = records
            .iter()
            .find(|r| r.observation.canonical_value == "prefer rust")
            .unwrap();
        assert_eq!(rust.distinct_conversations, 1);
        assert_eq!(rust.status, ObservationStatus::Candidate);
        Ok(())
    }
    #[test]
    fn system_boilerplate_and_short_greetings_do_not_merge_independent_prompts() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut store = Store::init(&temp.path().join("archive"))?;
        let chat = json!([{"role":"human","content":format!("<identity>\nYou are Kiro,\n{}", "provider boilerplate ".repeat(100))},
            {"role":"human","content":"I prefer concise explanations."},{"role":"bot","content":"Okay."}]);
        let first = session(temp.path(), "one", chat.clone())?;
        let second = session(temp.path(), "two", chat)?;
        store.import(&[first, second])?;
        analyze(&mut store)?;
        let records = store.observations(false)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].distinct_conversations, 2);
        Ok(())
    }
}
