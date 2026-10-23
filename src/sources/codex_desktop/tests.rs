use super::*;
use crate::{
    analysis::{AnalysisEngine, analyze},
    security::SecretKey,
    store::Store,
};
use rusqlite::params;

fn fixture(root: &Path) -> Result<()> {
    let state = Connection::open(root.join("state_5.sqlite"))?;
    state.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,title TEXT,cwd TEXT,source TEXT,thread_source TEXT,created_at INTEGER,updated_at INTEGER,archived INTEGER);")?;
    for (id, origin, source, archived) in [
        ("one", Some("user"), json!("vscode"), false),
        ("two", Some("user"), json!("vscode"), true),
        (
            "worker",
            Some("subagent"),
            json!({"subagent":{"thread_spawn":{}}}),
            false,
        ),
        ("review", Some("guardian_review"), json!("vscode"), false),
        ("unknown", None, json!("future-client"), false),
        ("missing", Some("user"), json!("vscode"), false),
    ] {
        state.execute(
            "INSERT INTO threads VALUES(?1,?1,?2,?3,?4,1,2,?5)",
            params![
                id,
                format!("/synthetic/project-{id}"),
                source.to_string(),
                origin,
                archived
            ],
        )?;
    }
    let history = Connection::open(root.join("thread_history_1.sqlite"))?;
    history.execute_batch("CREATE TABLE thread_items(thread_id TEXT,turn_id TEXT,item_id TEXT,rollout_ordinal INTEGER,item_json TEXT,item_type TEXT);
        CREATE TABLE thread_realtime_items(thread_id TEXT,item_id TEXT,rollout_ordinal INTEGER,item_json TEXT,item_type TEXT);")?;
    for id in ["one", "two", "worker", "review", "unknown"] {
        let item = json!({"type":"userMessage","id":format!("{id}-user"),"content":[{"type":"text","text":"Keep code simple."}]});
        history.execute(
            "INSERT INTO thread_items VALUES(?1,'turn',?2,0,?3,'userMessage')",
            params![id, format!("{id}-user"), item.to_string()],
        )?;
    }
    let item = json!({"type":"userMessage","id":"later-user","content":[{"type":"text","text":"I prefer Rust."}]});
    history.execute(
        "INSERT INTO thread_items VALUES('one','later-turn','later-user',2,?1,'userMessage')",
        [item.to_string()],
    )?;
    let item = json!({"type":"agentMessage","id":"reply","text":"A synthetic reply."});
    history.execute(
        "INSERT INTO thread_realtime_items VALUES('one','reply',1,?1,'agentMessage')",
        [item.to_string()],
    )?;
    Ok(())
}

#[test]
fn imports_multiple_projects_archives_and_realtime_without_changing_sources() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    fs::create_dir(temp.path().join("sessions"))?; // Empty directories are valid.
    let before = fs::read(temp.path().join("thread_history_1.sqlite"))?;
    let batch = read(temp.path())?;
    assert_eq!(batch.coverage.indexed_threads, 6);
    assert_eq!(batch.coverage.threads_without_history, 1);
    assert_eq!(batch.coverage.worker_threads, 2);
    assert_eq!(inspect(temp.path())?.threads_with_history, 5);
    assert_eq!(batch.conversations.len(), 5);
    let one = batch
        .conversations
        .iter()
        .find(|c| c.source_id == "one")
        .unwrap();
    assert_eq!(one.messages.len(), 3);
    assert_eq!(one.messages[0].source_id, "one-user");
    assert_eq!(one.messages[1].role, "assistant");
    assert_eq!(one.messages[2].source_id, "later-user");
    assert!(one.messages.iter().all(|m| m.created_at.is_none()));
    assert!(crate::app_snapshot::is_partial(&one.metadata));
    assert_eq!(one.metadata["cwd"], "/synthetic/project-one");
    let archived = batch
        .conversations
        .iter()
        .find(|c| c.source_id == "two")
        .unwrap();
    assert_eq!(archived.metadata["archived"], true);
    for id in ["worker", "review", "unknown"] {
        let source = batch
            .conversations
            .iter()
            .find(|c| c.source_id == id)
            .unwrap();
        assert_eq!(
            super::super::evidence_coverage(source).eligible_user_messages,
            0
        );
    }
    assert_eq!(
        before,
        fs::read(temp.path().join("thread_history_1.sqlite"))?
    );
    Ok(())
}

#[test]
fn encrypted_reimports_preserve_revisions_and_exclusions() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let key = SecretKey::generate()?;
    let mut store = Store::init_encrypted(&temp.path().join("archive"), &key)?;
    let batch = read(temp.path())?;
    assert_eq!(store.import(&batch.conversations)?.inserted, 5);
    assert_eq!(
        store.import(&read(temp.path())?.conversations)?.unchanged,
        5
    );
    let summary = analyze(&mut store, AnalysisEngine::Rules)?;
    assert_eq!(summary.with_user_evidence, 2);
    assert_eq!(summary.without_user_evidence, 3);
    assert_eq!(summary.worker_threads, 2);
    assert_eq!(summary.processed, 5);
    let record = store.observations(false)?.remove(0);
    store.feedback(
        &record.id,
        crate::analysis_store::FeedbackAction::Correct,
        Some("Keep implementations small."),
    )?;
    let history = Connection::open(temp.path().join("thread_history_1.sqlite"))?;
    let item = json!({"type":"userMessage","id":"append","content":[{"type":"text","text":"I prefer SQLite."}]});
    history.execute(
        "INSERT INTO thread_items VALUES('one','new-turn','append',3,?1,'userMessage')",
        [item.to_string()],
    )?;
    assert_eq!(store.import(&read(temp.path())?.conversations)?.updated, 1);
    assert_eq!(analyze(&mut store, AnalysisEngine::Rules)?.processed, 1);
    let reviewed = store
        .observations(true)?
        .into_iter()
        .find(|r| r.id == record.id)
        .unwrap();
    assert_eq!(reviewed.display_text(), "Keep implementations small.");
    assert_eq!(reviewed.feedback_count, 1);
    let first = &batch.conversations[0];
    assert_eq!(store.snapshot(&first.stable_id(), Some(1))?, *first);
    store.exclude_conversation(&first.stable_id(), true)?;
    assert_eq!(store.import(&read(temp.path())?.conversations)?.skipped, 1);
    Ok(())
}

#[test]
fn full_rollouts_win_on_overlap_and_partial_import_cannot_erase_history() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let sessions = temp.path().join("archived_sessions");
    fs::create_dir(&sessions)?;
    fs::write(
        sessions.join("one.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":"one","source":"cli"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer Rust."}]}})
        ),
    )?;
    let batch = read(temp.path())?;
    assert_eq!(batch.conversations.len(), 5);
    assert_eq!(batch.coverage.rollout_sources, 1);
    let one = batch
        .conversations
        .iter()
        .find(|c| c.source_id == "one")
        .unwrap();
    assert!(!crate::app_snapshot::is_partial(&one.metadata));
    assert_eq!(one.messages[0].content, "I prefer Rust.");
    let mut store = Store::init(&temp.path().join("archive"))?;
    store.import(&batch.conversations)?;
    fs::remove_file(sessions.join("one.jsonl"))?;
    assert!(store.import(&read(temp.path())?.conversations).is_err());
    assert_eq!(store.snapshot(&one.stable_id(), None)?, *one);
    Ok(())
}

#[test]
fn malformed_items_and_unknown_schemas_fail_before_persistence() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let history = Connection::open(temp.path().join("thread_history_1.sqlite"))?;
    history.execute(
        "UPDATE thread_items SET item_json='invalid' WHERE thread_id='two'",
        [],
    )?;
    assert!(read(temp.path()).is_err());
    history.execute_batch("DROP TABLE thread_realtime_items")?;
    assert!(inspect(temp.path()).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_roots_and_databases() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("home");
    fs::create_dir(&root)?;
    fixture(&root)?;
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&root, &link)?;
    assert!(read(&link).is_err());
    assert!(inspect(&link).is_err());
    fs::rename(root.join("state_5.sqlite"), root.join("real.sqlite"))?;
    std::os::unix::fs::symlink(root.join("real.sqlite"), root.join("state_5.sqlite"))?;
    assert!(read(&root).is_err());
    Ok(())
}

#[test]
fn oversized_items_and_partial_history_loss_are_rejected() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let mut store = Store::init(&temp.path().join("archive"))?;
    let batch = read(temp.path())?;
    store.import(&batch.conversations)?;
    let history = Connection::open(temp.path().join("thread_history_1.sqlite"))?;
    history.execute("DELETE FROM thread_items WHERE item_id='later-user'", [])?;
    assert!(store.import(&read(temp.path())?.conversations).is_err());
    let original = batch
        .conversations
        .iter()
        .find(|c| c.source_id == "one")
        .unwrap();
    assert_eq!(store.snapshot(&original.stable_id(), None)?, *original);
    history.execute(
        "UPDATE thread_items SET item_json=CAST(zeroblob(16777217) AS TEXT) WHERE thread_id='one'",
        [],
    )?;
    assert!(
        read(temp.path())
            .err()
            .unwrap()
            .to_string()
            .contains("16 MiB")
    );
    Ok(())
}

#[test]
fn desktop_worker_origin_overrides_missing_rollout_attribution() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let sessions = temp.path().join("sessions");
    fs::create_dir(&sessions)?;
    fs::write(
        sessions.join("review.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":"review","source":"vscode"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer invented facts."}]}})
        ),
    )?;
    let batch = read(temp.path())?;
    let review = batch
        .conversations
        .iter()
        .find(|c| c.source_id == "review")
        .unwrap();
    assert_eq!(review.messages[0].role, "agent");
    assert!(super::super::evidence_coverage(review).worker_thread);
    Ok(())
}

#[test]
fn detection_reports_storage_coverage_without_parsing_message_bodies() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join(".codex");
    fs::create_dir(&root)?;
    fixture(&root)?;
    let expected = read(&root)?.conversations;
    assert_eq!(
        super::super::read_source(&root, SourceFormat::CodexDesktop)?,
        expected
    );
    let history = Connection::open(root.join("thread_history_1.sqlite"))?;
    history.execute("UPDATE thread_items SET item_json='not JSON'", [])?;
    let detected = super::super::detect_local(temp.path())?;
    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].format, SourceFormat::CodexDesktop);
    assert_eq!(
        detected[0].coverage.as_ref().unwrap().threads_with_history,
        5
    );
    assert!(read(&root).is_err());
    Ok(())
}
