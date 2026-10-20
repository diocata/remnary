//! Consistent snapshots, non-destructive restore and content-free diagnostics.
use crate::{
    security::{SecretKey, apply_key},
    store::Store,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub database_integrity: bool,
    pub foreign_keys_valid: bool,
    pub schema_version: i64,
    pub sqlite_version: String,
    pub cipher_version: String,
    pub pending_projection_cleanup: usize,
}

impl Store {
    pub fn doctor(&self) -> Result<HealthReport> {
        self.check_schema()?;
        verify_integrity(&self.connection)?;
        Ok(HealthReport {
            database_integrity: true,
            foreign_keys_valid: true,
            schema_version: self
                .connection
                .pragma_query_value(None, "user_version", |r| r.get(0))?,
            sqlite_version: self
                .connection
                .query_row("SELECT sqlite_version()", [], |r| r.get(0))?,
            cipher_version: self
                .connection
                .pragma_query_value(None, "cipher_version", |r| r.get(0))?,
            pending_projection_cleanup: self.pending_cleanup_count()?,
        })
    }

    /// The destination is new and immutable; never truncate or replace a file.
    /// Supply a key for every production backup, including a new recovery key
    /// if the backup should use a different key from the active database.
    pub fn backup_to(&self, output: &Path, key: Option<&SecretKey>) -> Result<()> {
        self.snapshot_to(output, key, false)
    }

    fn snapshot_to(
        &self,
        output: &Path,
        key: Option<&SecretKey>,
        reset_projections: bool,
    ) -> Result<()> {
        ensure!(
            fs::symlink_metadata(output).is_err(),
            "backup destination already exists; choose a new filename"
        );
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        ensure!(
            parent.is_dir(),
            "backup destination directory must already exist"
        );
        let temporary =
            tempfile::NamedTempFile::new_in(parent).context("create private backup stage")?;
        {
            let mut target = Connection::open(temporary.path()).context("open backup stage")?;
            if let Some(key) = key {
                apply_key(&target, key)?;
            }
            target.pragma_update(None, "temp_store", "MEMORY")?;
            {
                let backup = Backup::new(&self.connection, &mut target)
                    .context("start consistent backup")?;
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    ensure!(
                        Instant::now() < deadline,
                        "backup exceeded its 30-second limit; retry while the archive is idle"
                    );
                    match backup.step(128)? {
                        StepResult::Done => break,
                        StepResult::More => {}
                        StepResult::Busy | StepResult::Locked => {
                            std::thread::sleep(Duration::from_millis(25))
                        }
                        _ => anyhow::bail!("backup returned an unsupported progress state"),
                    }
                }
            }
            if reset_projections {
                // A restored archive must never remove files owned by the
                // original installation. Rebuild its vault explicitly instead.
                target.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE; DELETE FROM managed_files; DELETE FROM file_cleanup; COMMIT;")?;
            }
            verify_integrity(&target)?;
        }
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(output)
            .map_err(|e| e.error)
            .context("publish backup without replacing an existing file")?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    /// Restore into a directory without a database. A failed validation never
    /// overwrites an existing archive or modifies the selected backup.
    pub fn restore_new(data_dir: &Path, backup: &Path, key: Option<&SecretKey>) -> Result<Self> {
        ensure!(
            fs::symlink_metadata(data_dir.join("observr.sqlite3")).is_err(),
            "restore requires a new data directory; the existing archive was preserved"
        );
        ensure!(
            fs::symlink_metadata(backup)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false),
            "backup must be a regular file, never a symlink"
        );
        let connection = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("open selected backup")?;
        if let Some(key) = key {
            apply_key(&connection, key)?;
        }
        let source = Store { connection };
        source.check_schema()?;
        verify_integrity(&source.connection)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(data_dir)?;
        source.snapshot_to(&data_dir.join("observr.sqlite3"), key, true)?;
        match key {
            Some(key) => Self::open_encrypted(data_dir, key),
            None => Self::open(data_dir),
        }
    }
}

fn verify_integrity(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA integrity_check")?;
    let results = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(results == ["ok"], "database integrity verification failed");
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    ensure!(
        statement.query([])?.next()?.is_none(),
        "database contains invalid evidence or source references"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::{AnalysisEngine, analyze},
        analysis_store::FeedbackAction,
        import::read_chatgpt,
    };
    #[test]
    fn encrypted_backup_restore_preserves_evidence_feedback_and_tombstones() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let key = SecretKey::generate()?;
        let backup_key = SecretKey::generate()?;
        let mut store = Store::init_encrypted(&temp.path().join("source"), &key)?;
        let mut sources = read_chatgpt(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json"),
        )?;
        sources[0].messages[0].content.push_str("\nI prefer Rust.");
        store.import(&sources)?;
        analyze(&mut store, AnalysisEngine::Rules)?;
        let observations = store.observations(false)?;
        let confirmed = observations[0].id.clone();
        store.feedback(
            &confirmed,
            FeedbackAction::Correct,
            Some("My reviewed correction"),
        )?;
        let deleted = observations[1].id.clone();
        store.delete_observation(&deleted)?;
        let backup = temp.path().join("portable.db");
        store.backup_to(&backup, Some(&backup_key))?;
        assert!(store.backup_to(&backup, Some(&backup_key)).is_err());
        assert!(Store::restore_new(&temp.path().join("wrong"), &backup, Some(&key)).is_err());
        let restored =
            Store::restore_new(&temp.path().join("restored"), &backup, Some(&backup_key))?;
        assert_eq!(restored.stats()?.messages, store.stats()?.messages);
        assert_eq!(restored.analysis_run_count()?, store.analysis_run_count()?);
        assert_eq!(
            restored
                .observations(false)?
                .iter()
                .find(|o| o.id == confirmed)
                .unwrap()
                .display_text(),
            "My reviewed correction"
        );
        assert!(!restored.observations(true)?.iter().any(|o| o.id == deleted));
        assert!(restored.doctor()?.database_integrity);
        assert!(!restored.search("Rust", 10)?.is_empty());
        assert!(
            Store::restore_new(&temp.path().join("restored"), &backup, Some(&backup_key)).is_err()
        );
        Ok(())
    }

    #[test]
    fn restored_store_never_owns_original_external_projections() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut store = Store::init(&temp.path().join("source"))?;
        let sources = read_chatgpt(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json"),
        )?;
        store.import(&sources)?;
        let note = temp.path().join("external.md");
        fs::write(&note, "My generated transcript")?;
        store.register_managed_file(
            &note,
            &crate::model::digest(b"My generated transcript"),
            &[sources[0].stable_id()],
            &[],
        )?;
        let backup = temp.path().join("backup.db");
        store.backup_to(&backup, None)?;
        let mut restored = Store::restore_new(&temp.path().join("restore"), &backup, None)?;
        restored.delete_conversation(&sources[0].stable_id())?;
        assert_eq!(fs::read_to_string(note)?, "My generated transcript");
        Ok(())
    }
}
