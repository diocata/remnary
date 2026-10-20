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
    io::Read,
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
    /// Convert a compatible plaintext archive into a NEW encrypted copy.
    /// Source is opened read-only and is never migrated or rekeyed in place.
    /// SQLCipher's backup API cannot mix encrypted and plaintext databases.
    pub fn encrypt_plaintext_copy(data_dir: &Path, output: &Path, key: &SecretKey) -> Result<()> {
        let source_path = data_dir.join("observr.sqlite3");
        ensure!(
            fs::symlink_metadata(&source_path)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink()),
            "conversion source must be a regular archive file, not a symlink"
        );
        let mut header = [0u8; 16];
        fs::File::open(&source_path)?.read_exact(&mut header)?;
        ensure!(
            &header == b"SQLite format 3\0",
            "conversion source is not plaintext; use backup for encrypted archives"
        );
        let source = Connection::open_with_flags(&source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        source.pragma_update(None, "temp_store", "MEMORY")?;
        source.busy_timeout(Duration::from_secs(5))?;
        let application: i64 = source.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = source.pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            application == crate::store::APPLICATION_ID
                && (1..=crate::store::SCHEMA_VERSION).contains(&version),
            "conversion source is not a supported Observr archive"
        );
        verify_integrity(&source)?;
        ensure!(
            fs::symlink_metadata(output).is_err(),
            "conversion destination already exists; choose a new filename"
        );
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        ensure!(
            parent.is_dir(),
            "conversion destination directory must exist"
        );
        let temporary = tempfile::NamedTempFile::new_in(parent)
            .context("create private encrypted conversion stage")?;
        {
            let mut target = Connection::open_with_flags(
                temporary.path(),
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            apply_key(&target, key)?;
            target.busy_timeout(Duration::from_secs(5))?;
            let mut source_uri = reqwest::Url::from_file_path(fs::canonicalize(&source_path)?)
                .map_err(|_| anyhow::anyhow!("cannot represent conversion source as a file URI"))?;
            source_uri.query_pairs_mut().append_pair("mode", "ro");
            target.execute(
                "ATTACH DATABASE ?1 AS observr_plain KEY ''",
                [source_uri.as_str()],
            )?;
            let interrupt = target.get_interrupt_handle();
            let (completed, deadline) = std::sync::mpsc::channel::<()>();
            let watchdog = std::thread::spawn(move || {
                if matches!(
                    deadline.recv_timeout(Duration::from_secs(30)),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    interrupt.interrupt();
                }
            });
            let result = (|| -> Result<()> {
                let transaction = target.transaction()?;
                let attached = Some("observr_plain");
                let application: i64 =
                    transaction.pragma_query_value(attached, "application_id", |r| r.get(0))?;
                let version: i64 =
                    transaction.pragma_query_value(attached, "user_version", |r| r.get(0))?;
                ensure!(
                    application == crate::store::APPLICATION_ID
                        && (1..=crate::store::SCHEMA_VERSION).contains(&version),
                    "conversion source changed to an unsupported archive"
                );
                transaction
                    .query_row(
                        "SELECT sqlcipher_export('main', 'observr_plain')",
                        [],
                        |_| Ok(()),
                    )
                    .context("export plaintext into encrypted stage")?;
                transaction.pragma_update(None, "application_id", application)?;
                transaction.pragma_update(None, "user_version", version)?;
                transaction.commit()?;
                target.execute_batch("DETACH DATABASE observr_plain; PRAGMA foreign_keys=ON;")?;
                Ok(())
            })();
            let _ = completed.send(());
            let _ = watchdog.join();
            result?;
            let mut converted = Store { connection: target };
            converted.migrate()?;
            // The copy is independently owned. It must not remove source vaults.
            converted.connection.execute_batch(
                "BEGIN IMMEDIATE; DELETE FROM managed_files; DELETE FROM file_cleanup; COMMIT;",
            )?;
            converted.doctor()?;
        }
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(output)
            .map_err(|e| e.error)
            .context("publish encrypted copy without replacing an existing file")?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

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
            let mut copied = Store { connection: target };
            if reset_projections {
                copied.migrate()?;
                // A restored archive must never remove files owned by the
                // original installation. Rebuild its vault explicitly instead.
                copied.connection.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE; DELETE FROM managed_files; DELETE FROM file_cleanup; COMMIT;")?;
            }
            verify_integrity(&copied.connection)?;
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
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        let source = Store { connection };
        source.check_compatible_schema()?;
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
    fn restoring_an_old_encrypted_backup_migrates_only_the_new_archive() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let key = SecretKey::generate()?;
        let mut store = Store::init_encrypted(&temp.path().join("source"), &key)?;
        store.import(&read_chatgpt(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json"),
        )?)?;
        analyze(&mut store, AnalysisEngine::Rules)?;
        let id = store.observations(false)?[0].id.clone();
        store.feedback(&id, FeedbackAction::Confirm, None)?;
        // Reproduce a schema-2 backup produced before active heads existed.
        store
            .connection
            .execute_batch("DROP TABLE active_analysis_runs; PRAGMA user_version=2;")?;
        let backup = temp.path().join("old-backup.sqlite3");
        store.backup_to(&backup, Some(&key))?;
        let original = fs::read(&backup)?;
        let restored = Store::restore_new(&temp.path().join("restored"), &backup, Some(&key))?;
        assert_eq!(
            restored.doctor()?.schema_version,
            crate::store::SCHEMA_VERSION
        );
        assert_eq!(restored.observations(false)?[0].feedback_count, 1);
        assert_eq!(
            restored.observations(false)?[0].status,
            crate::analysis_store::ObservationStatus::UserConfirmed
        );
        assert!(!restored.search("Rust", 10)?.is_empty());
        assert_eq!(fs::read(backup)?, original);
        Ok(())
    }
    #[test]
    fn plaintext_version_one_conversion_upgrades_only_the_encrypted_copy() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let legacy = temp.path().join("legacy é #?' archive");
        fs::create_dir(&legacy)?;
        let path = legacy.join("observr.sqlite3");
        let connection = Connection::open(&path)?;
        connection.execute_batch(include_str!("schema.sql"))?;
        connection.pragma_update(None, "application_id", crate::store::APPLICATION_ID)?;
        connection.pragma_update(None, "user_version", 1)?;
        let source = crate::model::Conversation {
            provider: "chatgpt".into(),
            source_id: "legacy-source".into(),
            title: Some("Synthetic legacy archive".into()),
            created_at: None,
            updated_at: None,
            metadata: serde_json::json!({}),
            messages: vec![crate::model::Message {
                source_id: "legacy-message".into(),
                parent_source_id: None,
                role: "user".into(),
                content: "Legacy plaintextneedle. I prefer Rust.".into(),
                created_at: None,
                metadata: serde_json::json!({}),
            }],
        };
        let id = source.stable_id();
        connection.execute("INSERT INTO conversations(id,provider,source_id,current_revision) VALUES (?1,'chatgpt','legacy-source',1)",[&id])?;
        connection.execute("INSERT INTO conversation_revisions(conversation_id,revision,fingerprint,title,metadata) VALUES (?1,1,?2,'Synthetic legacy archive','{}')",rusqlite::params![id,source.fingerprint()?])?;
        connection.execute("INSERT INTO messages(conversation_id,revision,source_id,position,role,content,metadata) VALUES (?1,1,'legacy-message',0,'user','Legacy plaintextneedle. I prefer Rust.','{}')",[&id])?;
        drop(connection);
        let original = fs::read(&path)?;
        let key = SecretKey::generate()?;
        let output = temp.path().join("encrypted-copy.sqlite3");
        Store::encrypt_plaintext_copy(&legacy, &output, &key)?;
        assert_eq!(fs::read(&path)?, original);
        let unchanged = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version: i64 = unchanged.pragma_query_value(None, "user_version", |r| r.get(0))?;
        assert_eq!(version, 1);
        assert!(!String::from_utf8_lossy(&fs::read(&output)?).contains("plaintextneedle"));
        assert!(
            Store::restore_new(
                &temp.path().join("wrong-key"),
                &output,
                Some(&SecretKey::generate()?)
            )
            .is_err()
        );
        let mut restored = Store::restore_new(&temp.path().join("converted"), &output, Some(&key))?;
        assert_eq!(restored.stats()?.messages, 1);
        assert_eq!(restored.search("plaintextneedle", 10)?.len(), 1);
        assert_eq!(
            restored.doctor()?.schema_version,
            crate::store::SCHEMA_VERSION
        );
        assert!(Store::encrypt_plaintext_copy(&legacy, &output, &key).is_err());
        assert_eq!(analyze(&mut restored, AnalysisEngine::Rules)?.processed, 1);
        assert_eq!(
            restored.observations(false)?[0].observation.evidence[0].source_message_id,
            "legacy-message"
        );
        assert_eq!(fs::read(path)?, original);
        Ok(())
    }

    #[test]
    fn plaintext_conversion_preserves_reviews_and_never_owns_the_original_vault() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let mut store = Store::init(&source)?;
        store.import(&read_chatgpt(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json"),
        )?)?;
        analyze(&mut store, AnalysisEngine::Rules)?;
        let id = store.observations(false)?[0].id.clone();
        store.feedback(
            &id,
            FeedbackAction::Correct,
            Some("My reviewed synthetic preference"),
        )?;
        let vault = temp.path().join("vault");
        crate::knowledge::sync_vault(&mut store, &vault, "UTC")?;
        let profile = fs::read(vault.join("Profile.md"))?;
        let original = fs::read(source.join("observr.sqlite3"))?;
        let key = SecretKey::generate()?;
        let output = temp.path().join("copy.sqlite3");
        Store::encrypt_plaintext_copy(&source, &output, &key)?;
        assert_eq!(fs::read(source.join("observr.sqlite3"))?, original);
        let mut converted = Store::restore_new(&temp.path().join("restored"), &output, Some(&key))?;
        assert_eq!(
            converted.observations(false)?[0].display_text(),
            "My reviewed synthetic preference"
        );
        assert_eq!(converted.observations(false)?[0].feedback_count, 1);
        assert!(converted.doctor()?.database_integrity);
        converted.delete_observation(&id)?;
        assert_eq!(fs::read(vault.join("Profile.md"))?, profile);
        assert!(store.observations(false)?.iter().any(|r| r.id == id));
        Ok(())
    }
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
