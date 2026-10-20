//! Serialize application-managed filesystem mutations across CLI processes.

use crate::store::Store;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

pub struct ProjectionGuard {
    file: File,
}
impl Drop for ProjectionGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl Store {
    pub fn projection_lock(&self) -> Result<ProjectionGuard> {
        let db = self
            .connection
            .path()
            .context("projection operations require an on-disk store")?;
        let path = Path::new(db)
            .parent()
            .context("database has no parent directory")?
            .join(".projections.lock");
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(
                metadata.file_type().is_file(),
                "projection lock must be a regular file"
            );
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).context("open projection lock")?;
        FileExt::lock_exclusive(&file).context("lock generated projection operations")?;
        Ok(ProjectionGuard { file })
    }

    /// Record durable ownership before any source-bearing bytes reach a new
    /// target or staging file. Existing hashes stay valid until final commit.
    pub fn reserve_managed_note(
        &mut self,
        path: &Path,
        content_hash: &str,
        generated_hash: &str,
        conversation_ids: &[String],
        observation_ids: &[String],
    ) -> Result<()> {
        ensure!(
            [content_hash, generated_hash]
                .iter()
                .all(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())),
            "projection intent requires SHA-256 hashes"
        );
        let parent = fs::canonicalize(path.parent().context("projection path has no parent")?)
            .context("resolve projection parent")?;
        let absolute = parent.join(path.file_name().context("projection has no filename")?);
        let path = absolute.to_str().context("projection path must be UTF-8")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT content_hash FROM managed_files WHERE path = ?1",
                [path],
                |r| r.get(0),
            )
            .optional()?;
        if let Ok(metadata) = fs::symlink_metadata(&absolute) {
            ensure!(
                metadata.file_type().is_file() && existing.is_some(),
                "preexisting unowned files and symlinks cannot be reserved"
            );
        }
        let pending: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_cleanup WHERE path = ?1)",
            [path],
            |r| r.get(0),
        )?;
        ensure!(
            !pending,
            "projection has pending cleanup; run observr cleanup first"
        );
        for id in conversation_ids {
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversations c WHERE c.id = ?1 AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id))", [id], |r| r.get(0))?;
            ensure!(valid, "projection source is missing or excluded");
        }
        for id in observation_ids {
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM observations WHERE id = ?1 AND status NOT IN ('user_rejected','dismissed','superseded'))", [id], |r| r.get(0))?;
            ensure!(valid, "projection observation is missing or hidden");
        }
        transaction.execute("INSERT OR IGNORE INTO managed_files (path, content_hash, generated_hash) VALUES (?1, ?2, ?3)",params![path, content_hash, generated_hash])?;
        for id in conversation_ids {
            transaction.execute("INSERT OR IGNORE INTO managed_file_sources (path, conversation_id) VALUES (?1, ?2)", params![path,id])?;
        }
        for id in observation_ids {
            transaction.execute("INSERT OR IGNORE INTO managed_file_observations (path, observation_id) VALUES (?1, ?2)", params![path,id])?;
        }
        transaction.commit()?;
        Ok(())
    }
}
