use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(data_dir: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_remnary"))
        .arg("--insecure-plaintext")
        .arg("--data-dir")
        .arg(data_dir)
        .args(arguments)
        .output()
        .expect("run Remnary")
}

fn successful(data_dir: &Path, arguments: &[&str]) -> String {
    let output = run(data_dir, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 command output")
}

#[test]
fn imports_searches_and_exports_without_losing_edits() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt-branches.json");
    let source = fixture.to_str().unwrap();
    assert!(!run(&data_dir, &["status"]).status.success());
    successful(&data_dir, &["init"]);
    assert!(successful(&data_dir, &["import", source]).contains("1 new"));
    assert!(successful(&data_dir, &["import", source]).contains("1 unchanged"));
    let stats = successful(&data_dir, &["status"]);
    assert!(stats.contains("Conversations: 1"));
    assert!(stats.contains("Current messages: 4"));
    let found = successful(&data_dir, &["search", "alternative"]);
    assert!(found.contains("a-alternative"));
    assert!(found.contains("chatgpt:conversation-branches@1#"));
    let vault = temp.path().join("vault");
    let export_path = vault.to_str().unwrap();
    assert!(successful(&data_dir, &["export", export_path]).contains("1 new"));
    assert!(successful(&data_dir, &["export", export_path]).contains("1 unchanged"));
    let note = fs::read_dir(&vault)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&note, "My edited transcript\n").unwrap();
    let conflict = run(&data_dir, &["export", export_path]);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("preserved"));
    assert_eq!(fs::read_to_string(&note).unwrap(), "My edited transcript\n");
}
