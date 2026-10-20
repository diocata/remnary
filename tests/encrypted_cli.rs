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
