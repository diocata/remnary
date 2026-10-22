use super::*;
use crate::{
    analysis_store::{ObservationStatus, Processor},
    intelligence::extract_rules,
    model::{Conversation, Message, digest},
};
use serde_json::json;

fn source(id: &str, content: &str) -> Conversation {
    Conversation {
        provider: "synthetic".into(),
        source_id: id.into(),
        title: Some("Synthetic privacy test".into()),
        created_at: Some(1.0),
        updated_at: Some(2.0),
        metadata: json!({}),
        messages: vec![Message {
            source_id: "synthetic-message".into(),
            parent_source_id: None,
            role: "user".into(),
            content: content.into(),
            created_at: Some(1.5),
            metadata: json!({}),
        }],
    }
}

fn process(store: &mut Store, processor: &Processor) -> Result<()> {
    for target in store.analysis_targets(processor)? {
        let output = extract_rules(&target.conversation, target.revision);
        store.save_analysis(&target, processor, output)?;
    }
    Ok(())
}

fn count(store: &Store, table: &str) -> Result<usize> {
    Ok(store
        .connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })? as usize)
}

#[test]
fn source_deletion_removes_no_longer_needed_span_metadata() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(directory.path())?;
    let source = source("one", "I prefer Rust.");
    let source_id = source.stable_id();
    store.import(&[source])?;
    process(&mut store, &Processor::rules())?;
    let observation_id = store.observations(false)?[0].id.clone();
    store.delete_observation(&observation_id)?;
    assert_eq!(count(&store, "deleted_evidence_spans")?, 1);
    store.delete_conversation(&source_id)?;
    assert_eq!(count(&store, "deleted_evidence_spans")?, 0);
    assert_eq!(count(&store, "deleted_sources")?, 1);
    Ok(())
}

#[test]
fn source_deletion_keeps_shared_claim_but_removes_orphans_and_cascades() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(directory.path())?;
    let first = source("first", "I prefer Rust. I prefer SQLite.");
    let second = source("second", "I prefer Rust.");
    store.import(&[first.clone(), second])?;
    process(&mut store, &Processor::rules())?;
    assert_eq!(store.observations(false)?.len(), 2);
    assert_eq!(count(&store, "evidence")?, 3);
    let result = store.delete_conversation(&first.stable_id())?;
    assert_eq!(result, CleanupSummary::default());
    let remaining = store.observations(false)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].observation.evidence.len(), 1);
    assert_eq!(remaining[0].distinct_conversations, 1);
    assert_eq!(remaining[0].status, ObservationStatus::Candidate);
    assert!(remaining[0].observation.summary.contains("Rust"));
    assert_eq!(count(&store, "evidence")?, 1);
    assert_eq!(count(&store, "analysis_runs")?, 1);
    assert_eq!(count(&store, "observation_extractions")?, 1);
    assert_eq!(count(&store, "deleted_observations")?, 1);
    assert!(store.search("SQLite", 10)?.is_empty());
    assert_eq!(store.import(&[first])?.skipped, 1);
    assert_eq!(store.stats()?.conversations, 1);
    Ok(())
}

#[test]
fn manual_observation_deletion_survives_a_new_processor() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(directory.path())?;
    store.import(&[source("first", "I prefer Rust.")])?;
    let mut processor = Processor::rules();
    process(&mut store, &processor)?;
    let id = store.observations(false)?[0].id.clone();
    store.delete_observation(&id)?;
    assert!(store.observations(true)?.is_empty());
    assert_eq!(count(&store, "evidence")?, 0);
    assert_eq!(count(&store, "observation_extractions")?, 0);
    processor.version = "privacy-test-next-version".into();
    process(&mut store, &processor)?;
    assert!(store.observations(true)?.is_empty());
    assert_eq!(count(&store, "deleted_observations")?, 1);
    assert!(store.delete_observation("missing").is_err());
    Ok(())
}

#[test]
fn exclusion_hides_evidence_and_restoration_recalculates_recurrence() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(directory.path())?;
    let first = source("first", "I prefer Rust.");
    store.import(&[first.clone(), source("second", "I prefer Rust.")])?;
    process(&mut store, &Processor::rules())?;
    assert_eq!(
        store.observations(false)?[0].status,
        ObservationStatus::Supported
    );
    store.exclude_conversation(&first.stable_id(), true)?;
    assert_eq!(store.excluded_conversations()?, vec![first.stable_id()]);
    assert_eq!(store.search("Rust", 10)?.len(), 1);
    let mut next = Processor::rules();
    next.version = "privacy-test-next-version".into();
    assert_eq!(store.analysis_targets(&next)?.len(), 1);
    let records = store.observations(false)?;
    assert_eq!(records[0].observation.evidence.len(), 1);
    assert_eq!(records[0].status, ObservationStatus::Candidate);
    assert_eq!(store.import(std::slice::from_ref(&first))?.skipped, 1);
    store.exclude_conversation(&first.stable_id(), false)?;
    assert!(store.excluded_conversations()?.is_empty());
    assert_eq!(store.search("Rust", 10)?.len(), 2);
    assert_eq!(
        store.observations(false)?[0].status,
        ObservationStatus::Supported
    );
    assert!(store.exclude_conversation("missing", true).is_err());
    Ok(())
}

#[test]
fn generated_files_are_removed_and_edited_files_remain_pending() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    let normal = directory.path().join("generated.md");
    let edited = directory.path().join("edited.md");
    fs::write(&normal, "generated original")?;
    fs::write(&edited, "editable original")?;
    store.register_managed_file(
        &normal,
        &digest(b"generated original"),
        std::slice::from_ref(&id),
        &[],
    )?;
    store.register_managed_file(
        &edited,
        &digest(b"editable original"),
        std::slice::from_ref(&id),
        &[],
    )?;
    fs::write(&edited, "My manual content")?;
    let summary = store.delete_conversation(&id)?;
    assert_eq!(
        summary,
        CleanupSummary {
            removed: 1,
            conflicts: 1
        }
    );
    assert!(!normal.exists());
    assert_eq!(fs::read_to_string(&edited)?, "My manual content");
    assert_eq!(store.pending_cleanup_count()?, 1);
    assert_eq!(store.process_cleanup()?.conflicts, 1);
    assert!(
        store
            .register_managed_file(&edited, &digest(b"My manual content"), &[], &[])
            .is_err()
    );
    fs::remove_file(&edited)?;
    assert_eq!(store.process_cleanup()?, CleanupSummary::default());
    assert_eq!(store.pending_cleanup_count()?, 0);
    Ok(())
}

#[test]
fn exclusion_and_shared_evidence_changes_invalidate_observation_only_files() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let second = source("second", "I prefer Rust.");
    store.import(&[first.clone(), second])?;
    process(&mut store, &Processor::rules())?;
    let id = store.observations(false)?[0].id.clone();
    let path = directory.path().join("shared.md");
    fs::write(&path, "Two supporting conversations")?;
    store.register_managed_file(
        &path,
        &digest(b"Two supporting conversations"),
        &[],
        std::slice::from_ref(&id),
    )?;
    store.exclude_conversation(&first.stable_id(), true)?;
    assert!(!path.exists());
    assert_eq!(store.pending_cleanup_count()?, 0);
    store.exclude_conversation(&first.stable_id(), false)?;
    fs::write(&path, "Two supporting conversations")?;
    store.register_managed_file(&path, &digest(b"Two supporting conversations"), &[], &[id])?;
    assert_eq!(store.delete_conversation(&first.stable_id())?.removed, 1);
    assert!(!path.exists());
    assert_eq!(store.observations(false)?[0].distinct_conversations, 1);
    Ok(())
}

#[test]
fn observation_deletion_cleans_its_projection_and_unknown_hash_is_rejected() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    store.import(&[source("first", "I prefer Rust.")])?;
    process(&mut store, &Processor::rules())?;
    let id = store.observations(false)?[0].id.clone();
    let path = directory.path().join("profile.md");
    fs::write(&path, "Generated profile")?;
    assert!(
        store
            .register_managed_file(&path, &digest(b"wrong"), &[], std::slice::from_ref(&id))
            .is_err()
    );
    store.register_managed_file(
        &path,
        &digest(b"Generated profile"),
        &[],
        std::slice::from_ref(&id),
    )?;
    assert_eq!(store.delete_observation(&id)?.removed, 1);
    assert!(!path.exists());
    assert_eq!(store.stats()?.conversations, 1);
    Ok(())
}

#[test]
fn deletion_sql_failure_rolls_back_tombstones_and_cleanup_registration() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    process(&mut store, &Processor::rules())?;
    let path = directory.path().join("generated.md");
    fs::write(&path, "Original source projection")?;
    store.register_managed_file(
        &path,
        &digest(b"Original source projection"),
        std::slice::from_ref(&id),
        &[],
    )?;
    store.connection.execute_batch(
        "CREATE TEMP TRIGGER fail_delete BEFORE DELETE ON conversations
         BEGIN SELECT RAISE(ABORT, 'synthetic deletion failure'); END;",
    )?;
    assert!(store.delete_conversation(&id).is_err());
    assert_eq!(store.stats()?.conversations, 1);
    assert_eq!(store.observations(false)?.len(), 1);
    assert_eq!(count(&store, "deleted_sources")?, 0);
    assert_eq!(count(&store, "deleted_observations")?, 0);
    assert_eq!(count(&store, "managed_files")?, 1);
    assert_eq!(count(&store, "managed_file_sources")?, 1);
    assert_eq!(store.pending_cleanup_count()?, 0);
    assert_eq!(fs::read_to_string(path)?, "Original source projection");
    Ok(())
}

fn managed_note(prefix: &str, body: &str, suffix: &str) -> String {
    format!(
        "{prefix}<!-- remnary:generated:start -->\n{body}<!-- remnary:generated:end -->{suffix}"
    )
}

#[test]
fn managed_note_cleanup_preserves_changed_user_sections_exactly() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    let path = directory.path().join("profile.md");
    let body = "# Generated profile\n\nI prefer Rust.\n";
    let original = managed_note("My first note\n", body, "\nMore notes\n");
    fs::write(&path, &original)?;
    store.register_managed_note(
        &path,
        &digest(original.as_bytes()),
        &digest(body.as_bytes()),
        std::slice::from_ref(&id),
        &[],
    )?;
    assert_eq!(
        store.managed_note_state(&path)?,
        Some(digest(body.as_bytes()))
    );
    let edited = managed_note("New personal prefix\n\n", body, "\nUpdated suffix 日本語\n");
    fs::write(&path, edited)?;
    let result = store.delete_conversation(&id)?;
    assert_eq!(
        result,
        CleanupSummary {
            removed: 1,
            conflicts: 0
        }
    );
    assert_eq!(
        fs::read_to_string(&path)?,
        managed_note("New personal prefix\n\n", "", "\nUpdated suffix 日本語\n")
    );
    assert_eq!(store.pending_cleanup_count()?, 0);
    assert_eq!(store.managed_note_state(&path)?, Some(digest(b"")));
    Ok(())
}

#[test]
fn changed_generated_section_is_preserved_as_a_visible_cleanup_conflict() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    let path = directory.path().join("profile.md");
    let body = "Generated preference.\n";
    let original = managed_note("", body, "");
    fs::write(&path, &original)?;
    assert!(
        store
            .register_managed_note(
                &path,
                &digest(original.as_bytes()),
                &digest(b"different body"),
                std::slice::from_ref(&id),
                &[]
            )
            .is_err()
    );
    store.register_managed_note(
        &path,
        &digest(original.as_bytes()),
        &digest(body.as_bytes()),
        std::slice::from_ref(&id),
        &[],
    )?;
    let edited = managed_note("User notes\n", "Edited generated preference.\n", "");
    fs::write(&path, &edited)?;
    assert_eq!(store.delete_conversation(&id)?.conflicts, 1);
    assert_eq!(fs::read_to_string(&path)?, edited);
    assert_eq!(store.pending_cleanup_count()?, 1);
    // Restoring the original generated body resolves the conflict without
    // requiring restoration of the original outside user sections.
    fs::write(&path, managed_note("Keep these notes\n", body, ""))?;
    assert_eq!(store.process_cleanup()?.removed, 1);
    assert_eq!(
        fs::read_to_string(&path)?,
        managed_note("Keep these notes\n", "", "")
    );
    assert_eq!(store.pending_cleanup_count()?, 0);
    Ok(())
}

#[test]
fn managed_note_without_user_content_is_removed() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    let path = directory.path().join("profile.md");
    let body = "Generated preference.\n";
    let original = managed_note("\n", body, "\n");
    fs::write(&path, &original)?;
    store.register_managed_note(
        &path,
        &digest(original.as_bytes()),
        &digest(body.as_bytes()),
        std::slice::from_ref(&id),
        &[],
    )?;
    assert_eq!(store.delete_conversation(&id)?.removed, 1);
    assert!(!path.exists());
    assert_eq!(store.pending_cleanup_count()?, 0);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_replacements_and_symlink_registration_preserve_targets() -> Result<()> {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir()?;
    let mut store = Store::init(&directory.path().join("data"))?;
    let first = source("first", "I prefer Rust.");
    let id = first.stable_id();
    store.import(&[first])?;
    let target = directory.path().join("user.md");
    let path = directory.path().join("generated.md");
    fs::write(&target, "User-owned file")?;
    symlink(&target, &path)?;
    assert!(
        store
            .register_managed_file(
                &path,
                &digest(b"User-owned file"),
                std::slice::from_ref(&id),
                &[]
            )
            .is_err()
    );
    fs::remove_file(&path)?;
    fs::write(&path, "Generated original")?;
    store.register_managed_file(
        &path,
        &digest(b"Generated original"),
        std::slice::from_ref(&id),
        &[],
    )?;
    fs::remove_file(&path)?;
    symlink(&target, &path)?;
    assert_eq!(store.delete_conversation(&id)?.conflicts, 1);
    assert_eq!(fs::read_to_string(&target)?, "User-owned file");
    assert!(fs::symlink_metadata(&path)?.file_type().is_symlink());
    assert_eq!(store.pending_cleanup_count()?, 1);
    Ok(())
}
