//! SQLCipher keys and the explicit boundary between encrypted storage and
//! plaintext exports. Key material is never serialized, printed or put in SQL.
//!
//! SQLCipher's supported raw-key syntax and first-schema-read validation:
//! https://www.zetetic.net/sqlcipher/sqlcipher-api/#setting-the-key
//! Native Keychain access uses keyring's binary get_secret/set_secret API:
//! https://docs.rs/keyring/3.6.3/keyring/struct.Entry.html

use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Read};
use std::path::Path;

use anyhow::{Result, anyhow, ensure};
use rusqlite::Connection;
use zeroize::Zeroizing;

const KEY_BYTES: usize = 32;
const KEY_HEX_BYTES: usize = KEY_BYTES * 2;
const DATABASE_NAME: &str = "observr.sqlite3";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "dev.observr";

/// A 256-bit key with zeroization on drop. No serialization or implicit clone.
/// Debug deliberately reveals no key material. Zeroization does not protect
/// against an already compromised process, crash dumps or all compiler copies.
pub struct SecretKey {
    bytes: Zeroizing<[u8; KEY_BYTES]>,
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretKey([REDACTED])")
    }
}

impl SecretKey {
    /// Explicit recovery export. The caller chooses the location and must keep
    /// this file separately from encrypted database backups.
    pub fn write_new(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        let mut encoded = Zeroizing::new(String::with_capacity(65));
        for byte in self.bytes.iter() {
            use std::fmt::Write;
            write!(&mut *encoded, "{byte:02x}")
                .map_err(|_| anyhow!("cannot encode recovery key"))?;
        }
        encoded.push('\n');
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|_| {
            anyhow!("cannot create key file; choose a new file in an existing private directory")
        })?;
        file.write_all(encoded.as_bytes())
            .map_err(|_| anyhow!("cannot write key file"))?;
        file.sync_all()
            .map_err(|_| anyhow!("cannot persist key file"))?;
        #[cfg(unix)]
        File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .and_then(|parent| parent.sync_all())
        .map_err(|_| anyhow!("cannot persist key file directory"))?;
        Ok(())
    }

    pub fn generate() -> Result<Self> {
        let mut key = Self {
            bytes: Zeroizing::new([0; KEY_BYTES]),
        };
        getrandom::fill(key.bytes.as_mut())
            .map_err(|_| anyhow!("operating-system random key generation failed"))?;
        Ok(key)
    }

    /// Read exactly 64 hexadecimal characters, with an optional LF or CRLF.
    /// Unix key files must have mode 0600. Symlinks, devices, directories,
    /// oversized input and permissive files are rejected without echoing data.
    pub fn from_key_file(path: &Path) -> Result<Self> {
        let before = fs::symlink_metadata(path)
            .map_err(|_| anyhow!("cannot inspect encryption key file"))?;
        validate_key_file(&before)?;
        let mut file = File::open(path).map_err(|_| anyhow!("cannot open encryption key file"))?;
        let opened = file
            .metadata()
            .map_err(|_| anyhow!("cannot inspect opened encryption key file"))?;
        validate_key_file(&opened)?;
        ensure!(
            same_file(&before, &opened),
            "encryption key file changed while opening"
        );
        let mut encoded = Zeroizing::new(Vec::with_capacity(KEY_HEX_BYTES + 3));
        file.by_ref()
            .take((KEY_HEX_BYTES + 3) as u64)
            .read_to_end(&mut encoded)
            .map_err(|_| anyhow!("cannot read encryption key file"))?;
        let after = fs::symlink_metadata(path)
            .map_err(|_| anyhow!("cannot recheck encryption key file"))?;
        validate_key_file(&after)?;
        ensure!(
            same_file(&before, &after)
                && same_file(
                    &opened,
                    &file
                        .metadata()
                        .map_err(|_| anyhow!("cannot recheck opened encryption key file"))?
                ),
            "encryption key file changed while reading"
        );
        Self::from_hex(&encoded)
    }

    fn from_hex(encoded: &[u8]) -> Result<Self> {
        let encoded = encoded
            .strip_suffix(b"\r\n")
            .or_else(|| encoded.strip_suffix(b"\n"))
            .unwrap_or(encoded);
        ensure!(
            encoded.len() == KEY_HEX_BYTES,
            "encryption key file must contain exactly 64 hexadecimal characters and an optional newline"
        );
        let mut key = Self {
            bytes: Zeroizing::new([0; KEY_BYTES]),
        };
        for (index, pair) in encoded.as_chunks::<2>().0.iter().enumerate() {
            let high = hex_digit(pair[0]);
            let low = hex_digit(pair[1]);
            ensure!(
                high.is_some() && low.is_some(),
                "encryption key file contains invalid hexadecimal data"
            );
            key.bytes[index] = (high.unwrap() << 4) | low.unwrap();
        }
        Ok(key)
    }

    #[cfg(target_os = "macos")]
    fn from_keychain(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() == KEY_BYTES,
            "stored encryption key has an invalid length; restore the original key"
        );
        let mut key = Self {
            bytes: Zeroizing::new([0; KEY_BYTES]),
        };
        key.bytes.copy_from_slice(bytes);
        Ok(key)
    }

    /// SQLCipher's documented raw-key representation, cleared when dropped.
    /// Passing raw binary bytes directly would instead invoke passphrase/KDF
    /// behavior; SQLCipher requires the x'<64 hex characters>' representation.
    pub(crate) fn sqlcipher_key(&self) -> Zeroizing<String> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut raw = Zeroizing::new(String::with_capacity(KEY_HEX_BYTES + 3));
        raw.push_str("x'");
        for byte in self.bytes.iter() {
            raw.push(HEX[(byte >> 4) as usize] as char);
            raw.push(HEX[(byte & 0x0f) as usize] as char);
        }
        raw.push('\'');
        raw
    }
}

/// Key a newly opened SQLCipher connection before any schema/page operations.
/// Wrong keys, plaintext databases and non-SQLCipher builds fail closed.
/// The caller must discard the connection after any error from this function.
pub fn apply_key(connection: &Connection, key: &SecretKey) -> Result<()> {
    let raw = key.sqlcipher_key();
    // SAFETY: connection owns a live sqlite3 handle for this entire call.
    // SQLCipher synchronously consumes this valid 67-byte raw-key value; the
    // buffer stays alive and its length fits c_int. No borrowed pointer escapes.
    // Calling the supported C API avoids rusqlite pragma_update building an
    // additional SQL String containing key material or exposing it to tracing.
    let result = unsafe {
        rusqlite::ffi::sqlite3_key(
            connection.handle(),
            raw.as_ptr().cast(),
            raw.len() as std::ffi::c_int,
        )
    };
    ensure!(
        result == rusqlite::ffi::SQLITE_OK,
        "SQLCipher refused the encryption key"
    );
    let version: String = connection
        .pragma_query_value(None, "cipher_version", |row| row.get(0))
        .map_err(|_| {
            anyhow!("SQLCipher encryption is unavailable; an encrypted build is required")
        })?;
    ensure!(
        !version.trim().is_empty(),
        "SQLCipher encryption is unavailable; an encrypted build is required"
    );
    connection
        .pragma_update(None, "cipher_memory_security", true)
        .map_err(|_| anyhow!("cannot enable SQLCipher memory protection"))?;
    connection
        .pragma_update(None, "temp_store", "MEMORY")
        .map_err(|_| anyhow!("cannot restrict SQLite temporary storage to memory"))?;
    let memory: String = connection
        .pragma_query_value(None, "cipher_memory_security", |row| row.get(0))
        .map_err(|_| anyhow!("cannot verify SQLCipher memory protection"))?;
    let temporary: i64 = connection
        .pragma_query_value(None, "temp_store", |row| row.get(0))
        .map_err(|_| anyhow!("cannot verify SQLite temporary storage policy"))?;
    let plaintext_header: String = connection
        .pragma_query_value(None, "cipher_plaintext_header_size", |row| row.get(0))
        .map_err(|_| anyhow!("cannot verify SQLCipher encrypted-header policy"))?;
    ensure!(
        memory == "1" && temporary == 2 && plaintext_header == "0",
        "SQLCipher security settings could not be verified"
    );
    // Setting a key alone does not validate it. Reading the first schema page
    // proves a populated database can be decrypted using that key. This check
    // also rejects accidentally opening an existing plaintext archive as keyed.
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .map_err(|_| anyhow!("cannot unlock encrypted database: the key is incorrect, the file is damaged, or this is a legacy plaintext store"))?;
    Ok(())
}

/// Resolve an explicitly supplied private key file or the native macOS
/// Keychain key for this canonical data directory. Never create a replacement
/// key when a database already exists, even when invoked by `init`.
pub fn resolve_key(data_dir: &Path, key_file: Option<&Path>, create: bool) -> Result<SecretKey> {
    if let Some(path) = key_file {
        return SecretKey::from_key_file(path);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (data_dir, create);
        anyhow::bail!(
            "automatic OS key storage is currently supported on macOS; provide an explicit private --key-file"
        )
    }
    #[cfg(target_os = "macos")]
    {
        let canonical = prepare_key_directory(data_dir, create)?;
        let _lock = lock_key_directory(&canonical)?;
        let database_exists = match fs::symlink_metadata(canonical.join(DATABASE_NAME)) {
            Ok(metadata) => {
                ensure!(
                    metadata.file_type().is_file(),
                    "database must be a regular file, never a symlink"
                );
                true
            }
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(_) => return Err(anyhow!("cannot inspect database before key lookup")),
        };
        let account = crate::model::digest(canonical.as_os_str().as_encoded_bytes());
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account).map_err(|_| {
            anyhow!(
                "cannot access macOS Keychain; unlock it or provide an explicit private --key-file"
            )
        })?;
        match entry.get_secret() {
            Ok(bytes) => SecretKey::from_keychain(&Zeroizing::new(bytes)),
            Err(keyring::Error::NoEntry) if create && !database_exists => {
                let key = SecretKey::generate()?;
                entry.set_secret(key.bytes.as_ref()).map_err(|_| {
                    anyhow!("cannot save encryption key in macOS Keychain; no database was created")
                })?;
                // Verify successful persistence before the caller creates a DB.
                let stored = Zeroizing::new(entry.get_secret().map_err(|_| {
                    anyhow!(
                        "cannot verify encryption key in macOS Keychain; no database was created"
                    )
                })?);
                ensure!(
                    stored.as_slice() == key.bytes.as_ref(),
                    "macOS Keychain key verification failed; no database was created"
                );
                Ok(key)
            }
            Err(keyring::Error::NoEntry) if database_exists => Err(anyhow!(
                "the original encryption key is missing for this existing database; restore the key or supply the correct private --key-file"
            )),
            Err(keyring::Error::NoEntry) => Err(anyhow!(
                "no encryption key is stored for this directory; run `observr init` first or supply the correct private --key-file"
            )),
            Err(_) => Err(anyhow!(
                "cannot read macOS Keychain; unlock it or provide an explicit private --key-file"
            )),
        }
    }
}

#[cfg(target_os = "macos")]
fn prepare_key_directory(path: &Path, create: bool) -> Result<std::path::PathBuf> {
    if create {
        let mut directory = fs::DirBuilder::new();
        directory.recursive(true);
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
        directory
            .create(path)
            .map_err(|_| anyhow!("cannot create private data directory"))?;
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| anyhow!("data directory is unavailable; run `observr init` first"))?;
    ensure!(canonical.is_dir(), "data directory is not a directory");
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn lock_key_directory(path: &Path) -> Result<File> {
    // OS advisory locks are released on process exit, avoiding stale lockfiles
    // after a crash. They serialize first-time Keychain creation across app
    // processes, because keyring's set_secret can overwrite an existing entry.
    let lock_path = path.join(".keychain.lock");
    match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => validate_key_file(&metadata)?,
        Err(error) if error.kind() == ErrorKind::NotFound => (),
        Err(_) => return Err(anyhow!("cannot inspect key initialization lock")),
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
    let lock = options
        .open(&lock_path)
        .map_err(|_| anyhow!("cannot open key initialization lock"))?;
    let metadata = lock
        .metadata()
        .map_err(|_| anyhow!("cannot inspect key initialization lock"))?;
    validate_key_file(&metadata)?;
    let current = fs::symlink_metadata(&lock_path)
        .map_err(|_| anyhow!("cannot recheck key initialization lock"))?;
    validate_key_file(&current)?;
    ensure!(
        same_file(&metadata, &current),
        "key initialization lock changed"
    );
    lock.try_lock().map_err(|_| anyhow!("encryption key initialization is busy or unavailable; retry after the other Observr process finishes"))?;
    Ok(lock)
}

fn validate_key_file(metadata: &Metadata) -> Result<()> {
    ensure!(
        metadata.file_type().is_file(),
        "encryption key must be a regular file, never a symlink or device"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o777 == 0o600,
            "encryption key file must have private permissions 0600"
        );
    }
    Ok(())
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

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SYNTHETIC_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path)?.write_all(bytes)?;
        Ok(())
    }

    #[test]
    fn key_file_requires_exact_hex_and_only_an_optional_line_ending() -> Result<()> {
        let directory = tempfile::tempdir()?;
        for (index, ending) in ["", "\n", "\r\n"].into_iter().enumerate() {
            let path = directory.path().join(format!("valid-{index}.key"));
            private_file(&path, format!("{SYNTHETIC_HEX}{ending}").as_bytes())?;
            let key = SecretKey::from_key_file(&path)?;
            assert_eq!(key.sqlcipher_key().as_str(), format!("x'{SYNTHETIC_HEX}'"));
            assert_eq!(format!("{key:?}"), "SecretKey([REDACTED])");
        }
        for (index, invalid) in [
            "too-short".to_owned(),
            format!(" {SYNTHETIC_HEX}"),
            format!("{SYNTHETIC_HEX} "),
            format!("{SYNTHETIC_HEX}\n\n"),
            "g".repeat(64),
            "0".repeat(10000),
        ]
        .into_iter()
        .enumerate()
        {
            let path = directory.path().join(format!("invalid-{index}.key"));
            private_file(&path, invalid.as_bytes())?;
            assert!(SecretKey::from_key_file(&path).is_err());
        }
        assert!(SecretKey::from_key_file(directory.path()).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn key_file_rejects_symlinks_and_nonprivate_permissions() -> Result<()> {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("private.key");
        private_file(&path, SYNTHETIC_HEX.as_bytes())?;
        let link = directory.path().join("link.key");
        symlink(&path, &link)?;
        assert!(SecretKey::from_key_file(&link).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert!(SecretKey::from_key_file(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        assert!(SecretKey::from_key_file(&path).is_ok());
        Ok(())
    }

    #[test]
    fn explicit_key_resolution_does_not_touch_a_native_keystore() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let key_path = directory.path().join("private.key");
        private_file(&key_path, SYNTHETIC_HEX.as_bytes())?;
        let absent_data = directory.path().join("not-created");
        let key = resolve_key(&absent_data, Some(&key_path), false)?;
        assert_eq!(key.sqlcipher_key().as_str(), format!("x'{SYNTHETIC_HEX}'"));
        assert!(!absent_data.exists());
        Ok(())
    }

    #[test]
    fn encrypted_database_and_fts_require_the_original_key() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("encrypted.db");
        let key = SecretKey::generate()?;
        let wrong = SecretKey::generate()?;
        let connection = Connection::open(&path)?;
        apply_key(&connection, &key)?;
        connection.execute_batch(
            "CREATE TABLE records (content TEXT NOT NULL);
             INSERT INTO records VALUES ('synthetic-sensitive-fts-evidence');
             CREATE VIRTUAL TABLE search USING fts5(content);
             INSERT INTO search SELECT content FROM records;",
        )?;
        drop(connection);
        let disk = fs::read(&path)?;
        assert!(!disk.starts_with(b"SQLite format 3"));
        assert!(
            !disk
                .windows(b"synthetic-sensitive-fts-evidence".len())
                .any(|part| part == b"synthetic-sensitive-fts-evidence")
        );
        let plain = Connection::open(&path)?;
        assert!(
            plain
                .query_row("SELECT count(*) FROM sqlite_master", [], |row| row
                    .get::<_, i64>(0))
                .is_err()
        );
        drop(plain);
        let incorrect = Connection::open(&path)?;
        assert!(apply_key(&incorrect, &wrong).is_err());
        drop(incorrect);
        let correct = Connection::open(&path)?;
        apply_key(&correct, &key)?;
        let results: i64 = correct.query_row(
            "SELECT count(*) FROM search WHERE search MATCH 'evidence'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(results, 1);
        Ok(())
    }

    #[test]
    fn legacy_plaintext_database_is_rejected_without_modification() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("plaintext.db");
        let connection = Connection::open(&path)?;
        connection
            .execute_batch("CREATE TABLE legacy (id INTEGER); INSERT INTO legacy VALUES (7);")?;
        drop(connection);
        let before = fs::read(&path)?;
        let keyed = Connection::open(&path)?;
        assert!(apply_key(&keyed, &SecretKey::generate()?).is_err());
        drop(keyed);
        assert_eq!(fs::read(&path)?, before);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn key_initialization_lock_is_private_and_released_when_dropped() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let first = lock_key_directory(directory.path())?;
        assert!(lock_key_directory(directory.path()).is_err());
        drop(first);
        assert!(lock_key_directory(directory.path()).is_ok());
        Ok(())
    }
}
