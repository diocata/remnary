use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{Result, ensure};
use remnary::{
    analysis::{AnalysisEngine, analyze},
    analysis_store::{FeedbackAction, ObservationStatus},
    local_sources::{SourceFormat, read_source},
    security::SecretKey,
    store::Store,
};
use serde_json::json;

#[test]
fn local_sources_survive_encrypted_reimport_review_and_revision_changes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let key = SecretKey::generate()?;
    let mut store = Store::init_encrypted(&temp.path().join("data"), &key)?;
    let fixtures = [
        (SourceFormat::ClaudeCode, "claude.jsonl", json!({"type":"user","sessionId":"claude","uuid":"m1","message":{"role":"user","content":"Keep implementations simple."}}).to_string()),
        (SourceFormat::Codex, "codex.jsonl", format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":"codex","source":"cli"}}),json!({"type":"response_item","ordinal":1,"payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Keep code simple."}]}}))),
        (SourceFormat::Kiro, "kiro.chat", json!({"executionId":"kiro","chat":[{"role":"human","content":"<identity>\nYou are Kiro, an AI assistant.\nDo not add tests."},{"role":"human","content":"Keep the code simple.\n<EnvironmentContext>\nI prefer invented environment instructions.\n</EnvironmentContext>"}]}).to_string()),
    ];
    for (format, name, bytes) in fixtures {
        let path = temp.path().join(name);
        fs::write(&path, &bytes)?;
        let sources = read_source(&path, format)?;
        assert_eq!(store.import(&sources)?.inserted, 1);
        assert_eq!(store.import(&sources)?.unchanged, 1);
        assert_eq!(fs::read_to_string(&path)?, bytes);
    }
    assert_eq!(analyze(&mut store, AnalysisEngine::Rules)?.processed, 3);
    let records = store.observations(false)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].distinct_conversations, 3);
    assert_eq!(records[0].occurrences, 3);
    let id = records[0].id.clone();
    store.feedback(
        &id,
        FeedbackAction::Correct,
        Some("Prefer simple implementations for prototypes."),
    )?;
    assert_eq!(analyze(&mut store, AnalysisEngine::Rules)?.processed, 0);
    let records = store.observations(false)?;
    assert_eq!(records[0].status, ObservationStatus::UserConfirmed);
    assert_eq!(records[0].feedback_count, 1);
    assert_eq!(
        records[0].display_text(),
        "Prefer simple implementations for prototypes."
    );
    let path = temp.path().join("kiro.chat");
    let mut changed: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    changed["chat"][1]["content"] = json!("I prefer SQLite.");
    fs::write(&path, serde_json::to_vec(&changed)?)?;
    assert_eq!(
        store
            .import(&read_source(&path, SourceFormat::Auto)?)?
            .updated,
        1
    );
    assert_eq!(analyze(&mut store, AnalysisEngine::Rules)?.processed, 1);
    let records = store.observations(false)?;
    assert_eq!(
        records
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .distinct_conversations,
        2
    );
    assert!(
        records
            .iter()
            .all(|r| !r.display_text().contains("invented"))
    );
    Ok(())
}

#[test]
fn detection_runs_without_initializing_or_unlocking_an_archive() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let unused = temp.path().join("must-not-be-created");
    let output = Command::new(env!("CARGO_BIN_EXE_remnary"))
        .arg("--data-dir")
        .arg(&unused)
        .args(["sources", "--detect", "--json"])
        .output()?;
    ensure!(output.status.success(), "inventory failed");
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout)?.is_array());
    assert!(!unused.exists());
    Ok(())
}

#[test]
fn app_snapshot_stdin_is_encrypted_without_creating_a_plaintext_source_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let path = temp.path().join("recovery.key");
    let key = SecretKey::generate()?;
    key.write_new(&path)?;
    drop(Store::init_encrypted(&data, &key)?);
    let snapshot = json!({"schemaVersion":1,"thread":{"id":"native-chatgpt","kind":"chatgpt","title":"Synthetic native view"},"page":{"order":"newest_first","hasMore":true},"turns":[{"id":"turn","items":[{"type":"userMessage","id":"user","content":[{"type":"text","text":"I prefer exact evidence in every observation."}]}]}]});
    let mut child = Command::new(env!("CARGO_BIN_EXE_remnary"))
        .arg("--data-dir")
        .arg(&data)
        .arg("--key-file")
        .arg(&path)
        .args(["import-app-snapshot", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    writeln!(child.stdin.take().unwrap(), "{snapshot}")?;
    let result = child.wait_with_output()?;
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8(result.stdout)?.contains("1 new"));
    let store = Store::open_encrypted(&data, &key)?;
    assert_eq!(store.stats()?.messages, 1);
    assert_eq!(store.search("exact evidence", 10)?.len(), 1);
    let bytes = fs::read(data.join("remnary.sqlite3"))?;
    assert!(!String::from_utf8_lossy(&bytes).contains("I prefer exact evidence"));
    assert_eq!(fs::read_dir(temp.path())?.count(), 2);
    Ok(())
}
