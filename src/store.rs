use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::model::{Conversation, Message};

const SCHEMA_VERSION: i64 = 3;
const APPLICATION_ID: i64 = 0x4f425352;
const DATABASE_NAME: &str = "observr.sqlite3";

pub struct Store {
    pub(crate) connection: Connection,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoreStats {
    pub conversations: usize,
    pub messages: usize,
    pub revisions: usize,
}

#[derive(Debug)]
pub struct SearchHit {
    pub provider: String,
    pub conversation_id: String,
    pub title: Option<String>,
    pub revision: i64,
    pub message_id: String,
    pub role: String,
    pub excerpt: String,
}

impl Store {
    /// Low-level plaintext storage for explicitly selected development use.
    pub fn init(data_dir: &Path) -> Result<Self> {
        Self::init_impl(data_dir, None)
    }

    pub fn init_encrypted(data_dir: &Path, key: &crate::security::SecretKey) -> Result<Self> {
        Self::init_impl(data_dir, Some(key))
    }

    fn init_impl(data_dir: &Path, key: Option<&crate::security::SecretKey>) -> Result<Self> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(data_dir)
            .context("creating data directory")?;
        let path = data_dir.join(DATABASE_NAME);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error).context("creating database file"),
        }
        let mut store = Self::connect(data_dir, key)?;
        let transaction = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let application: i64 =
            transaction.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let objects: i64 = transaction.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        // Recover after interruption between file creation and schema commit,
        // while never adopting a database that contains someone else's schema.
        if application == 0 && version == 0 && objects == 0 {
            transaction.execute_batch(include_str!("schema.sql"))?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
            transaction.pragma_update(None, "user_version", 1)?;
        }
        transaction.commit()?;
        store.migrate()?;
        store.check_schema()?;
        Ok(store)
    }

    pub fn open(data_dir: &Path) -> Result<Self> {
        Self::open_impl(data_dir, None)
    }

    pub fn open_encrypted(data_dir: &Path, key: &crate::security::SecretKey) -> Result<Self> {
        Self::open_impl(data_dir, Some(key))
    }

    fn open_impl(data_dir: &Path, key: Option<&crate::security::SecretKey>) -> Result<Self> {
        let mut store = Self::connect(data_dir, key)?;
        store.migrate()?;
        store.check_schema()?;
        Ok(store)
    }

    fn connect(data_dir: &Path, key: Option<&crate::security::SecretKey>) -> Result<Self> {
        let path = data_dir.join(DATABASE_NAME);
        ensure!(
            fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false),
            "database must be an existing regular file; run observr init first"
        );
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .with_context(|| format!("opening {}; run `observr init` first", path.display()))?;
        if let Some(key) = key {
            crate::security::apply_key(&connection, key)?;
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "secure_delete", true)?;
        Ok(Self { connection })
    }

    fn migrate(&mut self) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let application: i64 =
            transaction.pragma_query_value(None, "application_id", |r| r.get(0))?;
        ensure!(
            application == APPLICATION_ID,
            "this file is not an initialized Observr database"
        );
        let version: i64 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            (1..=SCHEMA_VERSION).contains(&version),
            "unsupported database schema {version}; expected 1..={SCHEMA_VERSION}"
        );
        if version < 2 {
            transaction.execute_batch(include_str!("migrations/002_intelligence.sql"))?;
            transaction.pragma_update(None, "user_version", 2)?;
        }
        if version < 3 {
            transaction.execute_batch(include_str!("migrations/003_active_analysis.sql"))?;
            transaction.pragma_update(None, "user_version", 3)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn check_schema(&self) -> Result<()> {
        let application: i64 = self
            .connection
            .pragma_query_value(None, "application_id", |r| r.get(0))?;
        ensure!(
            application == APPLICATION_ID,
            "this file is not an initialized Observr database"
        );
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            version == SCHEMA_VERSION,
            "unsupported database schema {version}; expected {SCHEMA_VERSION}"
        );
        Ok(())
    }

    /// The entire supplied batch commits together. Revisions are immutable snapshots.
    pub fn import(&mut self, conversations: &[Conversation]) -> Result<ImportSummary> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut summary = ImportSummary::default();
        for conversation in conversations {
            ensure!(
                !conversation.provider.is_empty() && !conversation.source_id.is_empty(),
                "conversation needs provider and source ID"
            );
            let id = conversation.stable_id();
            let suppressed: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM deleted_sources WHERE conversation_id = ?1
                               UNION ALL SELECT 1 FROM excluded_sources WHERE conversation_id = ?1)",
                [&id], |r| r.get(0),
            )?;
            if suppressed {
                summary.skipped += 1;
                continue;
            }
            let fingerprint = conversation.fingerprint()?;
            let existing: Option<(i64, String, Option<f64>)> = transaction.query_row(
                "SELECT c.current_revision, r.fingerprint, r.updated_at FROM conversations c
                 JOIN conversation_revisions r ON r.conversation_id = c.id AND r.revision = c.current_revision
                 WHERE c.id = ?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional()?;
            if existing
                .as_ref()
                .is_some_and(|(_, hash, _)| hash == &fingerprint)
            {
                summary.unchanged += 1;
                continue;
            }
            if crate::app_snapshot::is_partial(&conversation.metadata)
                && let Some((revision, _, _)) = existing.as_ref()
            {
                let metadata: String = transaction.query_row(
                    "SELECT metadata FROM conversation_revisions WHERE conversation_id = ?1 AND revision = ?2",
                    params![id,revision],|row|row.get(0))?;
                ensure!(
                    crate::app_snapshot::is_partial(&serde_json::from_str(&metadata)?),
                    "partial app view cannot replace a full provider export"
                );
                let incoming_ids = conversation
                    .messages
                    .iter()
                    .map(|m| m.source_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut statement = transaction.prepare(
                    "SELECT source_id FROM messages WHERE conversation_id = ?1 AND revision = ?2",
                )?;
                let old_ids = statement
                    .query_map(params![id, revision], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                ensure!(
                    old_ids.iter().all(|id| incoming_ids.contains(id.as_str())),
                    "partial app view would drop retained messages; combine source pages before importing"
                );
            }
            if existing.as_ref().is_some_and(|(_, _, time)| matches!((time, conversation.updated_at), (Some(current), Some(incoming)) if *current > incoming)) {
                summary.skipped += 1;
                continue;
            }
            let revision = match existing {
                Some((revision, _, _)) => {
                    summary.updated += 1;
                    revision + 1
                }
                None => {
                    summary.inserted += 1;
                    transaction.execute(
                        "INSERT INTO conversations (id, provider, source_id, current_revision) VALUES (?1, ?2, ?3, 1)",
                        params![id, conversation.provider, conversation.source_id],
                    )?;
                    1
                }
            };
            transaction.execute(
                "INSERT INTO conversation_revisions
                 (conversation_id, revision, fingerprint, title, created_at, updated_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    revision,
                    fingerprint,
                    conversation.title,
                    conversation.created_at,
                    conversation.updated_at,
                    serde_json::to_string(&conversation.metadata)?
                ],
            )?;
            for (position, message) in conversation.messages.iter().enumerate() {
                ensure!(!message.source_id.is_empty(), "message needs a source ID");
                transaction.execute(
                    "INSERT INTO messages
                     (conversation_id, revision, source_id, parent_source_id, position, role, content, created_at, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![id, revision, message.source_id, message.parent_source_id, position as i64,
                        message.role, message.content, message.created_at, serde_json::to_string(&message.metadata)?],
                )?;
            }
            transaction.execute(
                "UPDATE conversations SET current_revision = ?1 WHERE id = ?2",
                params![revision, id],
            )?;
        }
        transaction.commit()?;
        Ok(summary)
    }

    pub fn stats(&self) -> Result<StoreStats> {
        let count = |sql| -> Result<usize> {
            let count = self.connection.query_row(sql, [], |r| r.get::<_, i64>(0))?;
            Ok(usize::try_from(count)?)
        };
        Ok(StoreStats {
            conversations: count("SELECT count(*) FROM conversations")?,
            messages: count(
                "SELECT count(*) FROM messages m JOIN conversations c ON c.id = m.conversation_id AND c.current_revision = m.revision",
            )?,
            revisions: count("SELECT count(*) FROM conversation_revisions")?,
        })
    }

    /// Treat words as literal FTS tokens; user input is never an FTS expression.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        ensure!(
            (1..=1000).contains(&limit),
            "search limit must be between 1 and 1000"
        );
        let tokens: Vec<_> = query
            .split_whitespace()
            .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
            .collect();
        if tokens.is_empty() {
            bail!("search query must not be empty");
        }
        let mut statement = self.connection.prepare(
            "SELECT c.provider, c.source_id, r.title, m.revision, m.source_id, m.role,
                    snippet(messages_fts, 0, '[', ']', ' … ', 32)
             FROM messages_fts JOIN messages m ON m.id = messages_fts.rowid
             JOIN conversations c ON c.id = m.conversation_id AND c.current_revision = m.revision
             JOIN conversation_revisions r ON r.conversation_id = c.id AND r.revision = c.current_revision
             WHERE messages_fts MATCH ?1
             AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)
             ORDER BY rank, c.id, m.position LIMIT ?2",
        )?;
        let hits = statement
            .query_map(params![tokens.join(" AND "), limit as i64], |row| {
                Ok(SearchHit {
                    provider: row.get(0)?,
                    conversation_id: row.get(1)?,
                    title: row.get(2)?,
                    revision: row.get(3)?,
                    message_id: row.get(4)?,
                    role: row.get(5)?,
                    excerpt: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }

    pub fn conversations(&self) -> Result<Vec<Conversation>> {
        let mut statement = self.connection.prepare(
            "SELECT c.id, c.provider, c.source_id, c.current_revision, r.title, r.created_at, r.updated_at, r.metadata
             FROM conversations c JOIN conversation_revisions r
             ON r.conversation_id = c.id AND r.revision = c.current_revision
             WHERE NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)
             ORDER BY c.id",
        )?;
        let mut rows = statement.query([])?;
        let mut conversations = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let revision: i64 = row.get(3)?;
            let metadata: String = row.get(7)?;
            let mut messages = self.connection.prepare(
                "SELECT source_id, parent_source_id, role, content, created_at, metadata FROM messages
                 WHERE conversation_id = ?1 AND revision = ?2 ORDER BY position",
            )?;
            let message_rows = messages.query_map(params![id, revision], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?;
            let mut normalized_messages = Vec::new();
            for message in message_rows {
                let (source_id, parent_source_id, role, content, created_at, metadata) = message?;
                normalized_messages.push(Message {
                    source_id,
                    parent_source_id,
                    role,
                    content,
                    created_at,
                    metadata: serde_json::from_str(&metadata)?,
                });
            }
            conversations.push(Conversation {
                provider: row.get(1)?,
                source_id: row.get(2)?,
                title: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                metadata: serde_json::from_str(&metadata)?,
                messages: normalized_messages,
            });
        }
        Ok(conversations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn conversation(content: &str) -> Conversation {
        Conversation {
            provider: "chatgpt".into(),
            source_id: "c1".into(),
            title: Some("A project".into()),
            created_at: None,
            updated_at: None,
            metadata: json!({}),
            messages: vec![Message {
                source_id: "m1".into(),
                parent_source_id: None,
                role: "user".into(),
                content: content.into(),
                created_at: None,
                metadata: json!({}),
            }],
        }
    }

    #[test]
    fn deduplicates_and_preserves_revisions_but_searches_only_current_messages() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        let first = conversation("Use PostgreSQL for this project");
        assert_eq!(store.import(std::slice::from_ref(&first))?.inserted, 1);
        assert_eq!(store.import(std::slice::from_ref(&first))?.unchanged, 1);
        assert_eq!(store.search("PostgreSQL", 10)?.len(), 1);
        let changed = conversation("Use SQLite for this project");
        assert_eq!(store.import(std::slice::from_ref(&changed))?.updated, 1);
        assert!(store.search("PostgreSQL", 10)?.is_empty());
        assert_eq!(store.search("SQLite", 10)?[0].revision, 2);
        assert_eq!(
            store.stats()?,
            StoreStats {
                conversations: 1,
                messages: 1,
                revisions: 2
            }
        );
        assert_eq!(store.conversations()?, vec![changed]);
        drop(store);
        assert_eq!(Store::open(dir.path())?.stats()?.revisions, 2);
        assert_eq!(Store::init(dir.path())?.stats()?.revisions, 2);
        Ok(())
    }

    #[test]
    fn rolls_back_entire_import_on_duplicate_message_identity() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        let good = conversation("first");
        let mut bad = conversation("second");
        bad.source_id = "c2".into();
        bad.messages.push(bad.messages[0].clone());
        assert!(store.import(&[good, bad]).is_err());
        assert_eq!(store.stats()?.conversations, 0);
        assert!(store.search("first", 10)?.is_empty());
        Ok(())
    }

    #[test]
    fn search_is_literal_and_identity_is_scoped_to_provider_and_conversation() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        let first = conversation("Rust SQLite");
        let mut second = first.clone();
        second.provider = "claude".into();
        store.import(&[first, second])?;
        assert_eq!(store.stats()?.messages, 2);
        assert_eq!(store.search("rust sqlite", 10)?.len(), 2);
        assert!(store.search("\" OR *", 10).is_ok());
        assert!(store.search("   ", 10).is_err());
        assert!(store.search("rust", 0).is_err());
        Ok(())
    }

    #[test]
    fn refuses_uninitialized_or_newer_database() -> Result<()> {
        let dir = tempfile::tempdir()?;
        assert!(Store::open(dir.path()).is_err());
        assert!(!dir.path().join(DATABASE_NAME).exists());
        let store = Store::init(dir.path())?;
        store.connection.pragma_update(None, "user_version", 999)?;
        assert!(Store::open(dir.path()).is_err());
        Ok(())
    }

    #[test]
    fn version_two_migration_preserves_latest_analysis_head_across_vacuum() -> Result<()> {
        use crate::{
            analysis_store::Processor,
            intelligence::{AnalysisOutput, extract_rules},
        };
        let dir = tempfile::tempdir()?;
        let mut store = Store::init(dir.path())?;
        store.import(&[conversation("I prefer Rust.")])?;
        let processor = Processor::rules();
        let target = store.analysis_targets(&processor)?.remove(0);
        store.save_analysis(
            &target,
            &processor,
            extract_rules(&target.conversation, target.revision),
        )?;
        assert_eq!(store.observations(false)?.len(), 1);
        let replacement = Processor {
            version: "replacement".into(),
            ..processor
        };
        store.save_analysis(&target, &replacement, AnalysisOutput::default())?;
        store
            .connection
            .execute_batch("DROP TABLE active_analysis_runs; PRAGMA user_version = 2;")?;
        drop(store);
        let migrated = Store::open(dir.path())?;
        assert_eq!(migrated.analysis_run_count()?, 2);
        assert!(migrated.observations(false)?.is_empty());
        let selected: String =
            migrated
                .connection
                .query_row("SELECT run_id FROM active_analysis_runs", [], |r| r.get(0))?;
        migrated.connection.execute_batch("VACUUM")?;
        let after: String =
            migrated
                .connection
                .query_row("SELECT run_id FROM active_analysis_runs", [], |r| r.get(0))?;
        assert_eq!(selected, after);
        assert!(migrated.observations(false)?.is_empty());
        Ok(())
    }

    #[test]
    fn recovers_interrupted_initialization_and_refuses_foreign_schema() -> Result<()> {
        let empty = tempfile::tempdir()?;
        fs::File::create(empty.path().join(DATABASE_NAME))?;
        assert_eq!(Store::init(empty.path())?.stats()?.conversations, 0);
        let foreign = tempfile::tempdir()?;
        let connection = Connection::open(foreign.path().join(DATABASE_NAME))?;
        connection.execute_batch("CREATE TABLE other_application (id INTEGER)")?;
        drop(connection);
        assert!(Store::init(foreign.path()).is_err());
        Ok(())
    }
}
