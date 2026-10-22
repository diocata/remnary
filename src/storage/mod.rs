pub mod exclusions;
pub mod observations;
pub mod operations;
pub mod privacy;
pub mod projections;
pub mod security;

use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::model::{Conversation, Message};

pub(crate) const SCHEMA_VERSION: i64 = 5;
pub(crate) const APPLICATION_ID: i64 = 0x4f425352;
const DATABASE_NAME: &str = "remnary.sqlite3";
pub(crate) const LEGACY_DATABASE_NAME: &str = "observr.sqlite3";

// Keep existing archives usable without renaming live SQLite/WAL files.
pub(crate) fn database_path(data_dir: &Path) -> Result<std::path::PathBuf> {
    let current = data_dir.join(DATABASE_NAME);
    let legacy = data_dir.join(LEGACY_DATABASE_NAME);
    let current_exists = fs::symlink_metadata(&current).is_ok();
    let legacy_exists = fs::symlink_metadata(&legacy).is_ok();
    ensure!(
        !(current_exists && legacy_exists),
        "directory contains both current and legacy archive filenames; select an unambiguous archive directory"
    );
    Ok(if legacy_exists { legacy } else { current })
}

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
        let path = database_path(data_dir)?;
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
        let path = database_path(data_dir)?;
        ensure!(
            fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false),
            "database must be an existing regular file; run remnary init first"
        );
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .with_context(|| format!("opening {}; run `remnary init` first", path.display()))?;
        if let Some(key) = key {
            crate::security::apply_key(&connection, key)?;
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "secure_delete", true)?;
        Ok(Self { connection })
    }

    pub(crate) fn migrate(&mut self) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let application: i64 =
            transaction.pragma_query_value(None, "application_id", |r| r.get(0))?;
        ensure!(
            application == APPLICATION_ID,
            "this file is not an initialized Remnary database"
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
        if version < 4 {
            transaction.execute_batch(include_str!("migrations/004_exclusion_rules.sql"))?;
            transaction.pragma_update(None, "user_version", 4)?;
        }
        if version < 5 {
            transaction.execute_batch(include_str!("migrations/005_deleted_evidence_spans.sql"))?;
            transaction.pragma_update(None, "user_version", 5)?;
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
            "this file is not an initialized Remnary database"
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

    /// Read-only gate for old backups; migration happens on a private copy.
    pub(crate) fn check_compatible_schema(&self) -> Result<()> {
        let application: i64 = self
            .connection
            .pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            application == APPLICATION_ID,
            "this file is not an initialized Remnary database"
        );
        ensure!(
            (1..=SCHEMA_VERSION).contains(&version),
            "unsupported database schema {version}; supported 1..={SCHEMA_VERSION}"
        );
        Ok(())
    }

    /// The entire supplied batch commits together. Revisions are immutable snapshots.
    pub fn import(&mut self, conversations: &[Conversation]) -> Result<ImportSummary> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut summary = ImportSummary::default();
        let rules = crate::exclusions::read_rules(&transaction)?;
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
            if suppressed || rules.iter().any(|rule| rule.matches(conversation)) {
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
mod tests;
