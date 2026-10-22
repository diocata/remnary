//! Explicit source exclusions, tombstoned deletion and durable file cleanup.
//!
//! SQL changes commit atomically with cleanup instructions. Filesystem cleanup
//! follows the commit, so interruption cannot forget a projection that still
//! needs removing. Edited files and symlinks remain untouched and pending.
//! This does not remove independently copied files, provider exports, backups
//! or guarantee forensic erasure from the filesystem or storage device.

use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rusqlite::{Transaction, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    knowledge::{GENERATED_END, GENERATED_START, split_managed_section},
    model::digest,
    store::Store,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CleanupSummary {
    /// Generated projections removed, including sections stripped from notes
    /// with user content. Already-missing files simply complete the task.
    pub removed: usize,
    /// Pending files preserved because they changed, are unsafe or inaccessible.
    pub conflicts: usize,
}

fn remember_deleted_evidence_spans(transaction: &Transaction<'_>, id: &str) -> Result<()> {
    let mut query = transaction.prepare(
        "SELECT e.conversation_id,e.message_id,e.quote,m.content
        FROM evidence e JOIN messages m ON m.conversation_id=e.conversation_id
        AND m.revision=e.revision AND m.source_id=e.message_id WHERE e.observation_id=?1",
    )?;
    let mut rows = query.query([id])?;
    while let Some(row) = rows.next()? {
        let conversation: String = row.get(0)?;
        let message: String = row.get(1)?;
        let quote: String = row.get(2)?;
        let content: String = row.get(3)?;
        ensure!(!quote.is_empty(), "stored deletion evidence is empty");
        let fingerprint = digest(content.as_bytes());
        for (start, _) in content.match_indices(&quote) {
            transaction.execute("INSERT OR IGNORE INTO deleted_evidence_spans
                (conversation_id,message_id,message_digest,start_offset,end_offset) VALUES (?1,?2,?3,?4,?5)",
                params![conversation,message,fingerprint,start as i64,(start+quote.len()) as i64])?;
        }
    }
    Ok(())
}

pub(crate) type DeletedSpanCache = std::collections::BTreeMap<String, Vec<(i64, i64)>>;

pub(crate) fn reuses_deleted_evidence(
    transaction: &Transaction<'_>,
    source: &crate::model::Conversation,
    evidence: &crate::intelligence::EvidenceRef,
    cache: &mut DeletedSpanCache,
) -> Result<bool> {
    let message = source
        .messages
        .iter()
        .find(|message| message.source_id == evidence.source_message_id)
        .context("analysis references a missing source message")?;
    if !cache.contains_key(&message.source_id) {
        let has_spans: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM deleted_evidence_spans
            WHERE conversation_id=?1 AND message_id=?2)",
            params![evidence.conversation_id, evidence.source_message_id],
            |row| row.get(0),
        )?;
        let ranges = if has_spans {
            let fingerprint = digest(message.content.as_bytes());
            let mut query = transaction.prepare(
                "SELECT start_offset,end_offset FROM deleted_evidence_spans
                WHERE conversation_id=?1 AND message_id=?2 AND message_digest=?3",
            )?;
            query
                .query_map(
                    params![
                        evidence.conversation_id,
                        evidence.source_message_id,
                        fingerprint
                    ],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        cache.insert(message.source_id.clone(), ranges);
    }
    for &(deleted_start, deleted_end) in &cache[&message.source_id] {
        if message
            .content
            .match_indices(&evidence.quote)
            .any(|(start, _)| {
                (start as i64) < deleted_end
                    && ((start + evidence.quote.len()) as i64) > deleted_start
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Store {
    /// Exclusion keeps canonical evidence but hides the source and its evidence
    /// from search, extraction and generated knowledge. Existing projections
    /// are invalidated through the same durable cleanup queue as deletion.
    pub fn exclude_conversation(&mut self, stable_id: &str, excluded: bool) -> Result<()> {
        let _guard = self.projection_lock()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_source(&transaction, stable_id)?;
        if excluded {
            enqueue_source_files(&transaction, stable_id, "source_excluded")?;
            transaction.execute(
                "INSERT OR IGNORE INTO manual_excluded_sources (conversation_id) VALUES (?1)",
                [stable_id],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM manual_excluded_sources WHERE conversation_id = ?1",
                [stable_id],
            )?;
        }
        transaction.commit()?;
        // Conflicts remain observable through pending_cleanup_count and a
        // subsequent explicit process_cleanup call; no edited file is removed.
        self.process_cleanup_unlocked()?;
        Ok(())
    }

    pub fn excluded_conversations(&self) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT x.conversation_id FROM excluded_sources x
             JOIN conversations c ON c.id = x.conversation_id ORDER BY x.conversation_id",
        )?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Delete a claim, its evidence, extraction payloads and feedback while
    /// retaining opaque identity/span tombstones to suppress re-extraction from
    /// the cited unchanged source spans, including under new canonical wording.
    pub fn delete_observation(&mut self, id: &str) -> Result<CleanupSummary> {
        let _guard = self.projection_lock()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM observations WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )?;
        ensure!(exists, "observation not found");
        enqueue_observation_files(&transaction, id, "observation_deleted")?;
        remember_deleted_evidence_spans(&transaction, id)?;
        transaction.execute(
            "INSERT OR IGNORE INTO deleted_observations (observation_id) VALUES (?1)",
            [id],
        )?;
        transaction.execute("DELETE FROM observations WHERE id = ?1", [id])?;
        transaction.commit()?;
        self.process_cleanup_unlocked()
    }

    /// Delete every revision of one explicitly named source conversation.
    /// Shared observations retain their surviving evidence and feedback; claims
    /// with no remaining evidence are deleted and tombstoned in the same commit.
    pub fn delete_conversation(&mut self, stable_id: &str) -> Result<CleanupSummary> {
        let _guard = self.projection_lock()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_source(&transaction, stable_id)?;

        // Determine and enqueue all affected projections before deleting source
        // or evidence rows, including shared-claim notes whose counts change.
        enqueue_source_files(&transaction, stable_id, "source_deleted")?;
        let orphan_ids = {
            let mut statement = transaction.prepare(
                "SELECT o.id FROM observations o WHERE NOT EXISTS (
                    SELECT 1 FROM evidence e WHERE e.observation_id = o.id
                    AND e.conversation_id != ?1
                 ) ORDER BY o.id",
            )?;
            statement
                .query_map([stable_id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in &orphan_ids {
            enqueue_observation_files(&transaction, id, "source_deleted")?;
            transaction.execute(
                "INSERT OR IGNORE INTO deleted_observations (observation_id) VALUES (?1)",
                [id],
            )?;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO deleted_sources (conversation_id) VALUES (?1)",
            [stable_id],
        )?;
        transaction.execute(
            "DELETE FROM manual_excluded_sources WHERE conversation_id = ?1",
            [stable_id],
        )?;
        // Source revisions/messages, runs, extraction payloads and evidence
        // cascade through schema foreign keys. The FTS deletion trigger removes
        // the source from queries; this is not a forensic-erasure guarantee.
        // Once the source itself is gone, its whole-source tombstone is
        // sufficient. Retain no unnecessary message hashes or range metadata.
        transaction.execute(
            "DELETE FROM deleted_evidence_spans WHERE conversation_id = ?1",
            [stable_id],
        )?;
        transaction.execute("DELETE FROM conversations WHERE id = ?1", [stable_id])?;
        for id in &orphan_ids {
            transaction.execute("DELETE FROM observations WHERE id = ?1", [id])?;
        }
        transaction.commit()?;
        self.process_cleanup_unlocked()
    }

    /// Track a generated regular file and the records needed to recreate it.
    /// The caller must first preserve edits and write the intended projection.
    /// The expected digest is SHA-256, as produced by `model::digest`.
    pub fn register_managed_file(
        &mut self,
        path: &Path,
        content_hash: &str,
        conversation_ids: &[String],
        observation_ids: &[String],
    ) -> Result<()> {
        self.register_projection(path, content_hash, None, conversation_ids, observation_ids)
    }

    /// Register a note with an owned generated section and user-owned outside
    /// content. Cleanup validates the section digest rather than outside text.
    pub fn register_managed_note(
        &mut self,
        path: &Path,
        content_hash: &str,
        generated_hash: &str,
        conversation_ids: &[String],
        observation_ids: &[String],
    ) -> Result<()> {
        self.register_projection(
            path,
            content_hash,
            Some(generated_hash),
            conversation_ids,
            observation_ids,
        )
    }

    /// The registered generated-body digest for an existing managed note.
    /// Unregistered paths and whole-file transcript registrations return None.
    pub fn managed_note_state(&self, path: &Path) -> Result<Option<String>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("inspect managed note"),
        };
        ensure!(
            metadata.file_type().is_file(),
            "managed note must be a regular file, never a symlink"
        );
        let canonical = fs::canonicalize(path).context("canonicalize managed note")?;
        let canonical = canonical
            .to_str()
            .context("managed note path must be valid UTF-8")?;
        use rusqlite::OptionalExtension;
        Ok(self
            .connection
            .query_row(
                "SELECT generated_hash FROM managed_files WHERE path = ?1",
                [canonical],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    fn register_projection(
        &mut self,
        path: &Path,
        content_hash: &str,
        generated_hash: Option<&str>,
        conversation_ids: &[String],
        observation_ids: &[String],
    ) -> Result<()> {
        ensure!(
            content_hash.len() == 64 && content_hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "managed file requires a SHA-256 content hash"
        );
        ensure!(
            generated_hash.is_none_or(
                |hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            ),
            "managed generated section requires a SHA-256 content hash"
        );
        let metadata = fs::symlink_metadata(path).context("inspect managed file")?;
        ensure!(
            metadata.file_type().is_file(),
            "managed file must be a regular file, never a symlink"
        );
        let canonical = fs::canonicalize(path).context("canonicalize managed file")?;
        let (actual_hash, read_metadata) = hash_file(&canonical)?;
        if let Some(expected_body_hash) = generated_hash {
            let content = fs::read_to_string(&canonical).context("read managed note")?;
            let (_, body, _) = split_managed_section(&content)?;
            ensure!(
                digest(body.as_bytes()).eq_ignore_ascii_case(expected_body_hash),
                "managed generated section content hash does not match"
            );
        }
        let final_metadata = fs::symlink_metadata(path).context("recheck managed file")?;
        ensure!(
            final_metadata.file_type().is_file()
                && same_file(&metadata, &read_metadata)
                && same_file(&metadata, &final_metadata)
                && fs::canonicalize(path)? == canonical
                && actual_hash.eq_ignore_ascii_case(content_hash),
            "managed file changed or its content hash does not match"
        );
        let canonical = canonical
            .to_str()
            .context("managed file path must be valid UTF-8")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pending: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_cleanup WHERE path = ?1)",
            [canonical],
            |row| row.get(0),
        )?;
        ensure!(
            !pending,
            "managed file has pending deletion cleanup; resolve the conflict before registering it again"
        );
        for id in conversation_ids {
            require_source(&transaction, id)?;
            let excluded: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM excluded_sources WHERE conversation_id = ?1)",
                [id],
                |row| row.get(0),
            )?;
            ensure!(!excluded, "cannot register a file from an excluded source");
        }
        for id in observation_ids {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM observations WHERE id = ?1)",
                [id],
                |row| row.get(0),
            )?;
            ensure!(exists, "cannot register a file for an unknown observation");
        }
        transaction.execute(
            "INSERT INTO managed_files (path, content_hash, generated_hash) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET content_hash = excluded.content_hash,
             generated_hash = excluded.generated_hash",
            params![
                canonical,
                content_hash.to_ascii_lowercase(),
                generated_hash.map(str::to_ascii_lowercase)
            ],
        )?;
        transaction.execute(
            "DELETE FROM managed_file_sources WHERE path = ?1",
            [canonical],
        )?;
        transaction.execute(
            "DELETE FROM managed_file_observations WHERE path = ?1",
            [canonical],
        )?;
        for id in conversation_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO managed_file_sources (path, conversation_id) VALUES (?1, ?2)",
                params![canonical, id],
            )?;
        }
        for id in observation_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO managed_file_observations (path, observation_id) VALUES (?1, ?2)",
                params![canonical, id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn pending_cleanup_count(&self) -> Result<usize> {
        let count: i64 =
            self.connection
                .query_row("SELECT count(*) FROM file_cleanup", [], |row| row.get(0))?;
        Ok(usize::try_from(count)?)
    }

    /// Retry durable cleanup without overwriting or removing edited files.
    /// Missing files complete their entries; conflicts remain in the outbox.
    pub fn process_cleanup(&mut self) -> Result<CleanupSummary> {
        let _guard = self.projection_lock()?;
        self.process_cleanup_unlocked()
    }

    pub(crate) fn process_cleanup_unlocked(&mut self) -> Result<CleanupSummary> {
        let pending = {
            let mut statement = self.connection.prepare(
                "SELECT path, content_hash, generated_hash FROM file_cleanup ORDER BY path",
            )?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut summary = CleanupSummary::default();
        for (path, expected_hash, generated_hash) in pending {
            let mut scrubbed_hash = None;
            match clean_projection(Path::new(&path), &expected_hash, generated_hash.as_deref()) {
                CleanupOutcome::Removed => summary.removed += 1,
                CleanupOutcome::Scrubbed(hash) => {
                    summary.removed += 1;
                    scrubbed_hash = Some(hash);
                }
                CleanupOutcome::Missing => (),
                CleanupOutcome::Conflict => {
                    summary.conflicts += 1;
                    continue;
                }
            }
            let transaction = self.connection.transaction()?;
            if let Some(hash) = scrubbed_hash {
                // Keep ownership of the empty generated section, with no
                // deleted source references, so a later sync can safely reuse
                // this note without adopting arbitrary user-created files.
                transaction.execute(
                    "INSERT INTO managed_files(path,content_hash,generated_hash) VALUES (?1,?2,?3)
                     ON CONFLICT(path) DO UPDATE SET content_hash=excluded.content_hash,generated_hash=excluded.generated_hash",
                    params![path,hash,digest(b"")],
                )?;
            }
            transaction.execute(
                "DELETE FROM file_cleanup WHERE path = ?1 AND content_hash = ?2",
                params![path, expected_hash],
            )?;
            transaction.commit()?;
        }
        Ok(summary)
    }
}

fn require_source(transaction: &Transaction<'_>, id: &str) -> Result<()> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    ensure!(
        exists,
        "conversation not found; provide its stable Remnary ID"
    );
    Ok(())
}

pub(crate) fn enqueue_source_files(
    transaction: &Transaction<'_>,
    id: &str,
    reason: &str,
) -> Result<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO file_cleanup (path, content_hash, generated_hash, reason)
         SELECT f.path, f.content_hash, f.generated_hash, ?2 FROM managed_files f WHERE
           EXISTS(SELECT 1 FROM managed_file_sources s WHERE s.path = f.path AND s.conversation_id = ?1)
           OR EXISTS(SELECT 1 FROM managed_file_observations o JOIN evidence e
             ON e.observation_id = o.observation_id WHERE o.path = f.path AND e.conversation_id = ?1)",
        params![id, reason],
    )?;
    remove_enqueued_registrations(transaction)
}

pub(crate) fn enqueue_observation_files(
    transaction: &Transaction<'_>,
    id: &str,
    reason: &str,
) -> Result<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO file_cleanup (path, content_hash, generated_hash, reason)
         SELECT f.path, f.content_hash, f.generated_hash, ?2 FROM managed_files f
         WHERE EXISTS(SELECT 1 FROM managed_file_observations o
           WHERE o.path = f.path AND o.observation_id = ?1)",
        params![id, reason],
    )?;
    remove_enqueued_registrations(transaction)
}

fn remove_enqueued_registrations(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        "DELETE FROM managed_files WHERE path IN (SELECT path FROM file_cleanup)",
        [],
    )?;
    Ok(())
}

fn hash_file(path: &Path) -> Result<(String, Metadata)> {
    let mut file = File::open(path).context("open managed file")?;
    let before = file.metadata()?;
    ensure!(before.file_type().is_file(), "managed file is not regular");
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).context("read managed file")?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    ensure!(
        same_file(&before, &file.metadata()?),
        "managed file changed while reading"
    );
    Ok((format!("{:x}", hasher.finalize()), before))
}

fn same_file(first: &Metadata, second: &Metadata) -> bool {
    if first.len() != second.len() || first.modified().ok() != second.modified().ok() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if first.dev() != second.dev() || first.ino() != second.ino() {
            return false;
        }
    }
    true
}

enum CleanupOutcome {
    Removed,
    Scrubbed(String),
    Missing,
    Conflict,
}

fn clean_projection(
    path: &Path,
    expected_hash: &str,
    generated_hash: Option<&str>,
) -> CleanupOutcome {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return CleanupOutcome::Missing,
        Err(_) => return CleanupOutcome::Conflict,
    };
    if !path.is_absolute() || !metadata.file_type().is_file() {
        return CleanupOutcome::Conflict;
    }
    // Stored paths are canonical. Refuse a subsequently replaced ancestor
    // symlink rather than following it into another directory for deletion.
    if !matches!(fs::canonicalize(path), Ok(canonical) if canonical == path) {
        return CleanupOutcome::Conflict;
    }
    if let Some(generated_hash) = generated_hash {
        return scrub_managed_section(path, generated_hash, &metadata);
    }
    let (actual_hash, read_metadata) = match hash_file(path) {
        Ok(result) => result,
        Err(_) => return CleanupOutcome::Conflict,
    };
    let final_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return CleanupOutcome::Missing,
        Err(_) => return CleanupOutcome::Conflict,
    };
    if !final_metadata.file_type().is_file()
        || !same_file(&metadata, &read_metadata)
        || !same_file(&metadata, &final_metadata)
        || actual_hash != expected_hash
        || !matches!(fs::canonicalize(path), Ok(canonical) if canonical == path)
    {
        return CleanupOutcome::Conflict;
    }
    match fs::remove_file(path) {
        Ok(()) if sync_parent(path).is_ok() => CleanupOutcome::Removed,
        Ok(()) => CleanupOutcome::Conflict,
        Err(error) if error.kind() == ErrorKind::NotFound => CleanupOutcome::Missing,
        Err(_) => CleanupOutcome::Conflict,
    }
}

fn scrub_managed_section(path: &Path, expected_hash: &str, before: &Metadata) -> CleanupOutcome {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return CleanupOutcome::Conflict,
    };
    let (prefix, generated, suffix) = match split_managed_section(&content) {
        Ok(parts) => parts,
        Err(_) => return CleanupOutcome::Conflict,
    };
    if !generated.is_empty() && digest(generated.as_bytes()) != expected_hash {
        return CleanupOutcome::Conflict;
    }
    let outside = format!("{prefix}{suffix}");
    let safe_to_replace = || {
        matches!(fs::symlink_metadata(path), Ok(metadata)
            if metadata.file_type().is_file() && same_file(before, &metadata))
            && matches!(fs::canonicalize(path), Ok(canonical) if canonical == path)
    };
    if !safe_to_replace() {
        return CleanupOutcome::Conflict;
    }
    if outside.trim().is_empty() {
        return match fs::remove_file(path) {
            Ok(()) if sync_parent(path).is_ok() => CleanupOutcome::Removed,
            Ok(()) => CleanupOutcome::Conflict,
            Err(error) if error.kind() == ErrorKind::NotFound => CleanupOutcome::Missing,
            Err(_) => CleanupOutcome::Conflict,
        };
    }
    let scrubbed = format!("{prefix}{GENERATED_START}{GENERATED_END}{suffix}");
    if generated.is_empty() {
        // Recovery after a previous atomic scrub but before SQL acknowledgment.
        return CleanupOutcome::Scrubbed(digest(content.as_bytes()));
    }
    let Some(parent) = path.parent() else {
        return CleanupOutcome::Conflict;
    };
    let write = || -> Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(scrubbed.as_bytes())?;
        temporary.as_file().sync_all()?;
        ensure!(safe_to_replace(), "managed note changed during cleanup");
        temporary.persist(path).map_err(|error| error.error)?;
        sync_parent(path)?;
        Ok(())
    };
    match write() {
        Ok(()) => CleanupOutcome::Scrubbed(digest(scrubbed.as_bytes())),
        Err(_) => CleanupOutcome::Conflict,
    }
}

fn sync_parent(path: &Path) -> Result<()> {
    // On Unix, persist the directory entry before acknowledging filesystem
    // cleanup in SQLite. Other platforms still retain the durable SQL outbox;
    // filesystem durability needs target-specific verification before support.
    #[cfg(unix)]
    File::open(path.parent().context("managed file has no parent")?)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests;
