use super::*;
use serde_json::json;

fn conversation(content: &str) -> Conversation {
    Conversation {
        provider: "chatgpt".into(),
        source_id: "c1".into(),
        title: Some("A project".into()),
        created_at: None,
        updated_at: None,
        metadata: json!({}),
        messages: vec![Message {
            source_id: "m1".into(),
            parent_source_id: None,
            role: "user".into(),
            content: content.into(),
            created_at: None,
            metadata: json!({}),
        }],
    }
}

#[test]
fn deduplicates_and_preserves_revisions_but_searches_only_current_messages() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut store = Store::init(dir.path())?;
    let first = conversation("Use PostgreSQL for this project");
    assert_eq!(store.import(std::slice::from_ref(&first))?.inserted, 1);
    assert_eq!(store.import(std::slice::from_ref(&first))?.unchanged, 1);
    assert_eq!(store.search("PostgreSQL", 10)?.len(), 1);
    let changed = conversation("Use SQLite for this project");
    assert_eq!(store.import(std::slice::from_ref(&changed))?.updated, 1);
    assert!(store.search("PostgreSQL", 10)?.is_empty());
    assert_eq!(store.search("SQLite", 10)?[0].revision, 2);
    assert_eq!(
        store.stats()?,
        StoreStats {
            conversations: 1,
            messages: 1,
            revisions: 2
        }
    );
    assert_eq!(store.conversations()?, vec![changed]);
    drop(store);
    assert_eq!(Store::open(dir.path())?.stats()?.revisions, 2);
    assert_eq!(Store::init(dir.path())?.stats()?.revisions, 2);
    Ok(())
}

#[test]
fn rolls_back_entire_import_on_duplicate_message_identity() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut store = Store::init(dir.path())?;
    let good = conversation("first");
    let mut bad = conversation("second");
    bad.source_id = "c2".into();
    bad.messages.push(bad.messages[0].clone());
    assert!(store.import(&[good, bad]).is_err());
    assert_eq!(store.stats()?.conversations, 0);
    assert!(store.search("first", 10)?.is_empty());
    Ok(())
}

#[test]
fn search_is_literal_and_identity_is_scoped_to_provider_and_conversation() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut store = Store::init(dir.path())?;
    let first = conversation("Rust SQLite");
    let mut second = first.clone();
    second.provider = "claude".into();
    store.import(&[first, second])?;
    assert_eq!(store.stats()?.messages, 2);
    assert_eq!(store.search("rust sqlite", 10)?.len(), 2);
    assert!(store.search("\" OR *", 10).is_ok());
    assert!(store.search("   ", 10).is_err());
    assert!(store.search("rust", 0).is_err());
    Ok(())
}

#[test]
fn refuses_uninitialized_or_newer_database() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(Store::open(dir.path()).is_err());
    assert!(!dir.path().join(DATABASE_NAME).exists());
    let store = Store::init(dir.path())?;
    store.connection.pragma_update(None, "user_version", 999)?;
    assert!(Store::open(dir.path()).is_err());
    Ok(())
}

#[test]
fn version_two_migration_preserves_latest_analysis_head_across_vacuum() -> Result<()> {
    use crate::{
        analysis_store::Processor,
        intelligence::{AnalysisOutput, extract_rules},
    };
    let dir = tempfile::tempdir()?;
    let mut store = Store::init(dir.path())?;
    store.import(&[conversation("I prefer Rust.")])?;
    let processor = Processor::rules();
    let target = store.analysis_targets(&processor)?.remove(0);
    store.save_analysis(
        &target,
        &processor,
        extract_rules(&target.conversation, target.revision),
    )?;
    assert_eq!(store.observations(false)?.len(), 1);
    let replacement = Processor {
        version: "replacement".into(),
        ..processor
    };
    store.save_analysis(&target, &replacement, AnalysisOutput::default())?;
    store
        .connection
        .execute_batch("DROP TABLE deleted_evidence_spans; DROP VIEW excluded_sources; ALTER TABLE manual_excluded_sources RENAME TO excluded_sources; DROP TABLE exclusion_rules; DROP TABLE active_analysis_runs; PRAGMA user_version = 2;")?;
    drop(store);
    let migrated = Store::open(dir.path())?;
    assert_eq!(migrated.analysis_run_count()?, 2);
    assert!(migrated.observations(false)?.is_empty());
    let selected: String =
        migrated
            .connection
            .query_row("SELECT run_id FROM active_analysis_runs", [], |r| r.get(0))?;
    migrated.connection.execute_batch("VACUUM")?;
    let after: String =
        migrated
            .connection
            .query_row("SELECT run_id FROM active_analysis_runs", [], |r| r.get(0))?;
    assert_eq!(selected, after);
    assert!(migrated.observations(false)?.is_empty());
    Ok(())
}

#[test]
fn recovers_interrupted_initialization_and_refuses_foreign_schema() -> Result<()> {
    let empty = tempfile::tempdir()?;
    fs::File::create(empty.path().join(DATABASE_NAME))?;
    assert_eq!(Store::init(empty.path())?.stats()?.conversations, 0);
    let foreign = tempfile::tempdir()?;
    let connection = Connection::open(foreign.path().join(DATABASE_NAME))?;
    connection.execute_batch("CREATE TABLE other_application (id INTEGER)")?;
    drop(connection);
    assert!(Store::init(foreign.path()).is_err());
    Ok(())
}
