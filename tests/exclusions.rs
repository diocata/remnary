use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn encrypted_cli_exclusions_survive_backup_and_keep_manual_controls_independent() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let key = temp.path().join("key");
    let binary = env!("CARGO_BIN_EXE_observr");
    let created = Command::new(binary)
        .args(["key", "create"])
        .arg(&key)
        .output()
        .unwrap();
    assert!(created.status.success());
    let run = |directory: &Path, args: &[&str]| {
        let result = Command::new(binary)
            .arg("--data-dir")
            .arg(directory)
            .arg("--key-file")
            .arg(&key)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    run(&data, &["init"]);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json");
    run(&data, &["import", fixture.to_str().unwrap()]);
    let sources: Value = serde_json::from_str(&run(&data, &["sources", "--json"])).unwrap();
    let id = sources[0]["id"].as_str().unwrap();
    run(&data, &["exclude", id]);
    run(&data, &["privacy", "add", "provider", "ChatGPT"]);
    let rules: Value = serde_json::from_str(&run(&data, &["privacy", "list"])).unwrap();
    assert_eq!(rules[0]["value"], "chatgpt");
    assert_eq!(run(&data, &["sources", "--json"]).trim(), "[]");
    assert!(run(&data, &["import", fixture.to_str().unwrap()]).contains("2 skipped"));
    let backup = temp.path().join("backup.sqlite3");
    run(&data, &["backup", backup.to_str().unwrap()]);
    let restored = temp.path().join("restored");
    run(&restored, &["restore", backup.to_str().unwrap()]);
    run(
        &restored,
        &["privacy", "remove", rules[0]["id"].as_str().unwrap()],
    );
    let visible: Value = serde_json::from_str(&run(&restored, &["sources", "--json"])).unwrap();
    assert_eq!(visible.as_array().unwrap().len(), 1);
    run(&restored, &["include", id]);
    let visible: Value = serde_json::from_str(&run(&restored, &["sources", "--json"])).unwrap();
    assert_eq!(visible.as_array().unwrap().len(), 2);
    assert!(run(&restored, &["doctor"]).contains("\"schema_version\": 5"));
}
