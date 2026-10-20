//! Persistent exclusions apply to retained sources and future imports.
//! Topic matching is literal, ASCII case-insensitive substring matching.
//! Paths match recorded absolute paths with a component boundary, not globs.
use std::path::{Component, Path};

use anyhow::{Result, ensure};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{
    model::{Conversation, digest},
    privacy_store::enqueue_source_files,
    store::Store,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionScope {
    Provider,
    Project,
    Directory,
    Topic,
}
impl ExclusionScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Project => "project",
            Self::Directory => "directory",
            Self::Topic => "topic",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExclusionRule {
    pub id: String,
    pub scope: ExclusionScope,
    pub value: String,
}
impl ExclusionRule {
    pub fn new(scope: ExclusionScope, value: &str) -> Result<Self> {
        // Whitespace can be a real path component. Normalize only textual
        // provider/topic selectors, never silently select a different folder.
        let value = match scope {
            ExclusionScope::Provider | ExclusionScope::Topic => value.trim(),
            ExclusionScope::Directory | ExclusionScope::Project => value,
        };
        ensure!(
            !value.is_empty() && value.len() <= 4096 && !value.contains('\0'),
            "exclusion value must be 1–4096 bytes without NUL"
        );
        let value = match scope {
            ExclusionScope::Provider | ExclusionScope::Topic => value.to_ascii_lowercase(),
            ExclusionScope::Directory | ExclusionScope::Project => {
                let path = Path::new(value);
                ensure!(path.is_absolute(), "exclusion path must be absolute");
                ensure!(
                    !path.components().any(|c| matches!(c, Component::ParentDir)),
                    "exclusion path must not contain '..'"
                );
                let path: std::path::PathBuf = path.components().collect();
                path.to_str()
                    .ok_or_else(|| anyhow::anyhow!("exclusion path must be UTF-8"))?
                    .trim_end_matches('/')
                    .to_owned()
            }
        };
        let value = if value.is_empty() {
            "/".to_owned()
        } else {
            value
        };
        let id = digest(&serde_json::to_vec(&(scope, &value))?);
        Ok(Self { id, scope, value })
    }

    pub(crate) fn matches(&self, conversation: &Conversation) -> bool {
        let metadata = &conversation.metadata;
        let path_matches = |path: &str| {
            path == self.value
                || path
                    .strip_prefix(&self.value)
                    .is_some_and(|suffix| suffix.starts_with('/'))
                || self.value == "/" && path.starts_with('/')
        };
        let cwd_matches = || {
            metadata
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .is_some_and(path_matches)
        };
        match self.scope {
            ExclusionScope::Provider => conversation.provider.to_ascii_lowercase() == self.value,
            ExclusionScope::Topic => {
                conversation
                    .title
                    .as_ref()
                    .is_some_and(|s| s.to_ascii_lowercase().contains(&self.value))
                    || conversation
                        .messages
                        .iter()
                        .any(|m| m.content.to_ascii_lowercase().contains(&self.value))
            }
            ExclusionScope::Project => cwd_matches(),
            ExclusionScope::Directory => {
                cwd_matches()
                    || metadata
                        .get("source_path")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(path_matches)
                    || metadata
                        .get("source_paths")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|paths| {
                            paths
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .any(path_matches)
                        })
            }
        }
    }
}

pub(crate) fn read_rules(connection: &Connection) -> Result<Vec<ExclusionRule>> {
    let mut query =
        connection.prepare("SELECT id, scope, value FROM exclusion_rules ORDER BY id")?;
    let records = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|(id, scope, value)| {
            Ok(ExclusionRule {
                id,
                scope: serde_json::from_value(serde_json::Value::String(scope))?,
                value,
            })
        })
        .collect()
}

impl Store {
    pub fn exclusion_rules(&self) -> Result<Vec<ExclusionRule>> {
        read_rules(&self.connection)
    }

    /// Apply in the same transaction as durable projection invalidation.
    pub fn add_exclusion_rule(&mut self, rule: &ExclusionRule) -> Result<usize> {
        // Public fields can be deserialized/constructed by callers: validate too.
        let checked = ExclusionRule::new(rule.scope, &rule.value)?;
        ensure!(
            checked.id == rule.id && checked.value == rule.value,
            "exclusion rule is not normalized"
        );
        let _guard = self.projection_lock()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = excluded_ids(&transaction)?;
        transaction.execute(
            "INSERT OR IGNORE INTO exclusion_rules (id, scope, value) VALUES (?1, ?2, ?3)",
            params![rule.id, rule.scope.as_str(), rule.value],
        )?;
        let newly_excluded = excluded_ids(&transaction)?
            .into_iter()
            .filter(|id| !before.contains(id))
            .collect::<Vec<_>>();
        for id in &newly_excluded {
            enqueue_source_files(&transaction, id, "exclusion_rule")?;
        }
        transaction.commit()?;
        self.process_cleanup_unlocked()?;
        Ok(newly_excluded.len())
    }

    pub fn remove_exclusion_rule(&mut self, id: &str) -> Result<()> {
        let _guard = self.projection_lock()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute("DELETE FROM exclusion_rules WHERE id = ?1", [id])?;
        ensure!(changed == 1, "exclusion rule not found");
        transaction.commit()?;
        Ok(())
    }
}

fn excluded_ids(connection: &Connection) -> Result<std::collections::BTreeSet<String>> {
    let mut query = connection.prepare("SELECT conversation_id FROM excluded_sources")?;
    Ok(query
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::{AnalysisEngine, analyze},
        analysis_store::Processor,
        intelligence::extract_rules,
        model::Message,
    };
    use serde_json::json;

    fn source(id: &str, provider: &str, metadata: serde_json::Value, text: &str) -> Conversation {
        Conversation {
            provider: provider.into(),
            source_id: id.into(),
            title: Some("Synthetic discussion".into()),
            created_at: None,
            updated_at: None,
            metadata,
            messages: vec![Message {
                source_id: "one".into(),
                parent_source_id: None,
                role: "user".into(),
                content: text.into(),
                created_at: None,
                metadata: json!({}),
            }],
        }
    }

    #[test]
    fn rule_matchers_agree_with_sql_for_literals_unicode_and_path_boundaries() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(tmp.path())?;
        let conversations = vec![
            source(
                "one",
                "codex",
                json!({"cwd":"/private/project", "source_path":"/private/sessions/one.jsonl"}),
                "I prefer Rust. Budget 100%_safe. CAFÉ",
            ),
            source(
                "two",
                "chatgpt",
                json!({"cwd":"/private/project-other"}),
                "I prefer Go. CAFE",
            ),
            source(
                "three",
                "kiro",
                json!({"source_paths":["/private/sessions/sub/a.chat", "/other/b.chat"]}),
                "I prefer Rust.",
            ),
            source(
                "four",
                "claude-code",
                json!({"cwd":"/private/project/sub"}),
                "I prefer Go.",
            ),
            source(
                "five",
                "codex",
                json!({"source_path":"/private/sessions-other/file.jsonl"}),
                "I prefer Go.",
            ),
        ];
        store.import(&conversations)?;
        for (case, (scope, value)) in [
            (ExclusionScope::Provider, "CoDeX"),
            (ExclusionScope::Project, "/private/project"),
            (ExclusionScope::Directory, "/private/sessions/"),
            (ExclusionScope::Directory, "/"),
            (ExclusionScope::Topic, "100%_safe"),
            (ExclusionScope::Topic, "CAFÉ"),
            (ExclusionScope::Topic, "' OR 1=1 --"),
            (ExclusionScope::Topic, "synthetic discussion"),
        ]
        .into_iter()
        .enumerate()
        {
            let rule = ExclusionRule::new(scope, value)?;
            let expected = conversations
                .iter()
                .filter(|c| rule.matches(c))
                .map(Conversation::stable_id)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(store.add_exclusion_rule(&rule)?, expected.len());
            assert_eq!(
                store
                    .excluded_conversations()?
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>(),
                expected
            );
            let mut future = conversations.clone();
            for c in &mut future {
                c.source_id.push_str(&format!("-future-{case}"));
            }
            let result = store.import(&future)?;
            assert_eq!(result.skipped, expected.len());
            assert_eq!(
                result.inserted + result.unchanged,
                future.len() - expected.len()
            );
            // Remove any retained future sources before comparing the next rule.
            store.remove_exclusion_rule(&rule.id)?;
            for c in &future {
                if store
                    .current_snapshots()?
                    .iter()
                    .any(|s| s.conversation.stable_id() == c.stable_id())
                {
                    store.delete_conversation(&c.stable_id())?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn path_rules_preserve_significant_trailing_whitespace() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(tmp.path())?;
        let exact = source(
            "space",
            "codex",
            json!({"cwd":"/private/project "}),
            "hello",
        );
        let other = source(
            "no-space",
            "codex",
            json!({"cwd":"/private/project"}),
            "hello",
        );
        store.import(&[exact.clone(), other.clone()])?;
        for scope in [ExclusionScope::Project, ExclusionScope::Directory] {
            let rule = ExclusionRule::new(scope, "/private/project ")?;
            assert_eq!(rule.value, "/private/project ");
            assert!(rule.matches(&exact));
            assert!(!rule.matches(&other));
            assert_eq!(store.add_exclusion_rule(&rule)?, 1);
            assert_eq!(store.excluded_conversations()?, vec![exact.stable_id()]);
            store.remove_exclusion_rule(&rule.id)?;
        }
        Ok(())
    }

    #[test]
    fn overlapping_rules_and_manual_exclusions_survive_reopen_and_removal() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(tmp.path())?;
        let c = source("one", "codex", json!({}), "I prefer Rust.");
        store.import(std::slice::from_ref(&c))?;
        let provider = ExclusionRule::new(ExclusionScope::Provider, "codex")?;
        let topic = ExclusionRule::new(ExclusionScope::Topic, "Rust")?;
        assert_eq!(store.add_exclusion_rule(&provider)?, 1);
        assert_eq!(store.add_exclusion_rule(&topic)?, 0);
        store.exclude_conversation(&c.stable_id(), true)?;
        drop(store);
        let mut store = Store::open(tmp.path())?;
        assert_eq!(store.exclusion_rules()?.len(), 2);
        store.remove_exclusion_rule(&provider.id)?;
        store.exclude_conversation(&c.stable_id(), false)?;
        assert!(store.search("Rust", 10)?.is_empty());
        assert!(store.snapshot(&c.stable_id(), Some(1)).is_err());
        assert!(store.analysis_targets(&Processor::rules())?.is_empty());
        store.remove_exclusion_rule(&topic.id)?;
        assert_eq!(store.search("Rust", 10)?.len(), 1);
        assert!(store.remove_exclusion_rule(&topic.id).is_err());
        Ok(())
    }

    #[test]
    fn rules_invalidate_derived_files_and_inflight_analysis_without_deleting_evidence() -> Result<()>
    {
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(&tmp.path().join("data"))?;
        let c = source("one", "codex", json!({}), "I prefer Rust.");
        store.import(std::slice::from_ref(&c))?;
        let processor = Processor::rules();
        let target = store.analysis_targets(&processor)?.remove(0);
        analyze(&mut store, AnalysisEngine::Rules)?;
        let vault = tmp.path().join("vault");
        crate::knowledge::sync_vault(&mut store, &vault, "UTC")?;
        let note = tmp.path().join("projection.md");
        std::fs::write(&note, "Synthetic generated projection")?;
        store.register_managed_file(
            &note,
            &digest(b"Synthetic generated projection"),
            &[c.stable_id()],
            &[],
        )?;
        let rule = ExclusionRule::new(ExclusionScope::Provider, "codex")?;
        store.add_exclusion_rule(&rule)?;
        assert!(!note.exists());
        assert!(store.observations(true)?.is_empty());
        assert!(
            store
                .save_analysis(&target, &processor, extract_rules(&c, 1))
                .is_err()
        );
        assert_eq!(store.stats()?.conversations, 1);
        store.remove_exclusion_rule(&rule.id)?;
        assert_eq!(store.observations(false)?.len(), 1);
        assert!(!note.exists()); // Removal never silently recreates plaintext.
        Ok(())
    }

    #[test]
    fn schema_three_upgrade_keeps_manual_exclusions_and_backup_keeps_rules() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(&tmp.path().join("data"))?;
        let c = source("one", "codex", json!({}), "I prefer Rust.");
        store.import(std::slice::from_ref(&c))?;
        store.exclude_conversation(&c.stable_id(), true)?;
        store.connection.execute_batch("DROP TABLE deleted_evidence_spans; DROP VIEW excluded_sources; ALTER TABLE manual_excluded_sources RENAME TO excluded_sources; DROP TABLE exclusion_rules; PRAGMA user_version=3;")?;
        drop(store);
        let mut store = Store::open(&tmp.path().join("data"))?;
        assert_eq!(store.excluded_conversations()?, vec![c.stable_id()]);
        let rule = ExclusionRule::new(ExclusionScope::Provider, "claude-code")?;
        store.add_exclusion_rule(&rule)?;
        let backup = tmp.path().join("backup.sqlite3");
        store.backup_to(&backup, None)?;
        let restored = Store::restore_new(&tmp.path().join("restored"), &backup, None)?;
        assert_eq!(restored.exclusion_rules()?[0].id, rule.id);
        assert_eq!(restored.excluded_conversations()?, vec![c.stable_id()]);
        Ok(())
    }

    #[test]
    fn empty_relative_traversing_and_forged_rules_fail_closed() -> Result<()> {
        for value in ["", "   ", "secret\0topic"] {
            assert!(ExclusionRule::new(ExclusionScope::Topic, value).is_err());
        }
        for value in ["relative/path", "/private/../other"] {
            assert!(ExclusionRule::new(ExclusionScope::Directory, value).is_err());
        }
        let tmp = tempfile::tempdir()?;
        let mut store = Store::init(tmp.path())?;
        let mut rule = ExclusionRule::new(ExclusionScope::Provider, "codex")?;
        rule.id = "forged".into();
        assert!(store.add_exclusion_rule(&rule).is_err());
        assert!(store.exclusion_rules()?.is_empty());
        Ok(())
    }
}
