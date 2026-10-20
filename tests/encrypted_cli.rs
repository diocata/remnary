use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(data: &Path, key: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_observr"))
        .arg("--data-dir")
        .arg(data)
        .arg("--key-file")
        .arg(key)
        .args(args)
        .output()
        .unwrap()
}
fn ok(data: &Path, key: &Path, args: &[&str]) -> String {
    let result = run(data, key, args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
#[test]
fn encrypted_cli_workflow_survives_backup_restore_and_source_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let key = temp.path().join("recovery.key");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_observr"))
            .args(["key", "create"])
            .arg(&key)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_observr"))
            .args(["key", "create"])
            .arg(&key)
            .output()
            .unwrap()
            .status
            .code()
            == Some(1)
    );
    ok(&data, &key, &["init"]);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json");
    let fixture = fixture.to_str().unwrap();
    assert!(ok(&data, &key, &["import", fixture]).contains("2 new"));
    assert!(ok(&data, &key, &["analyze"]).contains("2 conversations"));
    assert!(ok(&data, &key, &["analyze"]).contains("No pending"));
    let discoveries: Vec<Value> =
        serde_json::from_str(&ok(&data, &key, &["discover", "--json"])).unwrap();
    let recurring = discoveries
        .iter()
        .find(|item| item["distinct_conversations"] == 2)
        .unwrap();
    let id = recurring["id"].as_str().unwrap();
    assert_eq!(recurring["status"], "supported");
    ok(
        &data,
        &key,
        &[
            "feedback",
            id,
            "correct",
            "--text",
            "Prefer simple implementations for personal projects",
        ],
    );
    let profile: Vec<Value> =
        serde_json::from_str(&ok(&data, &key, &["profile", "--json"])).unwrap();
    assert_eq!(profile.len(), 1);
    assert_eq!(profile[0]["status"], "user_confirmed");
    let vault = temp.path().join("vault");
    ok(&data, &key, &["vault", "sync", vault.to_str().unwrap()]);
    assert!(
        fs::read_to_string(vault.join("Profile.md"))
            .unwrap()
            .contains("Prefer simple implementations for personal projects")
    );
    let bytes = fs::read(data.join("observr.sqlite3")).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    assert!(
        !bytes
            .windows(b"Keep the implementation simple".len())
            .any(|part| part == b"Keep the implementation simple")
    );
    assert!(ok(&data, &key, &["doctor"]).contains("\"database_integrity\": true"));
    let backup = temp.path().join("backup.db");
    ok(&data, &key, &["backup", backup.to_str().unwrap()]);
    let restored = temp.path().join("restored");
    ok(&restored, &key, &["restore", backup.to_str().unwrap()]);
    let restored_profile: Vec<Value> =
        serde_json::from_str(&ok(&restored, &key, &["profile", "--json"])).unwrap();
    assert_eq!(restored_profile, profile);
    let sources: Vec<Value> =
        serde_json::from_str(&ok(&data, &key, &["sources", "--json"])).unwrap();
    let source = sources[0]["id"].as_str().unwrap();
    ok(&data, &key, &["exclude", source]);
    assert_eq!(
        serde_json::from_str::<Vec<Value>>(&ok(&data, &key, &["sources", "--json"]))
            .unwrap()
            .len(),
        1
    );
    ok(&data, &key, &["include", source]);
    ok(&data, &key, &["delete", "conversation", source]);
    assert!(ok(&data, &key, &["import", fixture]).contains("1 skipped"));
    assert!(ok(&data, &key, &["status"]).contains("Conversations: 1"));
    assert!(fs::read_to_string(vault.join("Profile.md")).is_err());
    assert!(
        !run(&data, &key, &["restore", backup.to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn plaintext_upgrade_and_key_rotation_publish_independent_verified_archives() {
    let temp = tempfile::tempdir().unwrap();
    let legacy = temp.path().join("legacy");
    let key = temp.path().join("new.key");
    let plain = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_observr"))
            .arg("--insecure-plaintext")
            .arg("--data-dir")
            .arg(&legacy)
            .args(args)
            .output()
            .unwrap()
    };
    for args in [
        vec!["init"],
        vec![
            "import",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chatgpt.json"),
        ],
        vec!["analyze"],
    ] {
        let result = plain(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let original = fs::read(legacy.join("observr.sqlite3")).unwrap();
    let key_result = Command::new(env!("CARGO_BIN_EXE_observr"))
        .args(["key", "create"])
        .arg(&key)
        .output()
        .unwrap();
    assert!(key_result.status.success());
    let encrypted = temp.path().join("encrypted.sqlite3");
    // Encryption conversion must be explicitly selected; no implicit adoption.
    assert!(
        !run(
            &legacy,
            &key,
            &[
                "encrypt-copy",
                encrypted.to_str().unwrap(),
                "--new-key-file",
                key.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    assert!(!encrypted.exists());
    let result = plain(&[
        "encrypt-copy",
        encrypted.to_str().unwrap(),
        "--new-key-file",
        key.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(legacy.join("observr.sqlite3")).unwrap(), original);
    let converted = temp.path().join("converted");
    ok(&converted, &key, &["restore", encrypted.to_str().unwrap()]);
    assert!(ok(&converted, &key, &["search", "Rust"]).contains("Rust"));
    assert!(ok(&converted, &key, &["doctor"]).contains("\"database_integrity\": true"));
    let rotated_key = temp.path().join("rotated.key");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_observr"))
            .args(["key", "create"])
            .arg(&rotated_key)
            .output()
            .unwrap()
            .status
            .success()
    );
    let rotated_backup = temp.path().join("rotated.sqlite3");
    ok(
        &converted,
        &key,
        &[
            "backup",
            rotated_backup.to_str().unwrap(),
            "--backup-key-file",
            rotated_key.to_str().unwrap(),
        ],
    );
    let rotated = temp.path().join("rotated");
    assert!(
        !run(
            &rotated,
            &key,
            &["restore", rotated_backup.to_str().unwrap()]
        )
        .status
        .success()
    );
    ok(
        &rotated,
        &rotated_key,
        &["restore", rotated_backup.to_str().unwrap()],
    );
    assert!(ok(&rotated, &rotated_key, &["doctor"]).contains("\"database_integrity\": true"));
    assert!(ok(&rotated, &rotated_key, &["status"]).contains("Conversations: 2"));
    assert!(!run(&rotated, &key, &["status"]).status.success());
    assert!(ok(&converted, &key, &["status"]).contains("Conversations: 2"));
    assert!(!run(&converted, &rotated_key, &["status"]).status.success());
}
