use anyhow::Result;
use observr::{
    analysis_store::{FeedbackAction, ObservationStatus, Processor},
    import::read_chatgpt,
    intelligence::{AnalysisOutput, extract_rules},
    store::Store,
};

fn fixture() -> Result<Vec<observr::model::Conversation>> {
    read_chatgpt(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json"),
    )
}

fn process(store: &mut Store, processor: &Processor) -> Result<()> {
    for target in store.analysis_targets(processor)? {
        let output = extract_rules(&target.conversation, target.revision);
        store.save_analysis(&target, processor, output)?;
    }
    Ok(())
}

#[test]
fn processor_replacement_retires_automatic_claims_but_preserves_human_review() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(&temp.path().join("data"))?;
    let mut sources = fixture()?;
    sources[0]
        .messages
        .iter_mut()
        .find(|m| m.role == "user")
        .unwrap()
        .content
        .push_str(" I prefer Rust.");
    store.import(&sources)?;
    let original = Processor::rules();
    process(&mut store, &original)?;
    let records = store.observations(false)?;
    let confirmed_id = records
        .iter()
        .find(|r| r.observation.canonical_value == "prefer rust")
        .unwrap()
        .id
        .clone();
    let automatic_id = records
        .iter()
        .find(|r| r.id != confirmed_id)
        .unwrap()
        .id
        .clone();
    store.feedback(
        &confirmed_id,
        FeedbackAction::Correct,
        Some("Rust for this project"),
    )?;
    let vault = temp.path().join("vault");
    observr::knowledge::sync_vault(&mut store, &vault, "UTC")?;
    let mut replacement = original.clone();
    replacement.version = "new-version-returning-no-claims".into();
    for target in store.analysis_targets(&replacement)? {
        store.save_analysis(&target, &replacement, AnalysisOutput::default())?;
    }
    let remaining = store.observations(true)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, confirmed_id);
    assert_eq!(remaining[0].display_text(), "Rust for this project");
    assert_eq!(remaining[0].feedback_count, 1);
    assert!(
        !vault
            .join("Candidates")
            .join(format!("{automatic_id}.md"))
            .exists()
    );
    assert!(store.analysis_targets(&replacement)?.is_empty());
    observr::knowledge::sync_vault(&mut store, &vault, "UTC")?;
    assert!(std::fs::read_to_string(vault.join("Profile.md"))?.contains("Rust for this project"));
    // History remains cached; selecting an already completed older run does
    // not silently reactivate its retired automatic candidates.
    process(&mut store, &original)?;
    assert_eq!(store.observations(true)?.len(), 1);
    drop(store);
    assert_eq!(
        Store::open(&temp.path().join("data"))?
            .observations(true)?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn incremental_analysis_groups_distinct_sources_and_preserves_feedback() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    store.import(&fixture()?)?;
    let processor = Processor::rules();
    process(&mut store, &processor)?;
    let observations = store.observations(false)?;
    let recurring = observations
        .iter()
        .find(|o| o.observation.canonical_value == "keep_implementations_simple")
        .expect("fixture contains the same correction in two conversations");
    assert_eq!(recurring.distinct_conversations, 2);
    assert_eq!(recurring.status, ObservationStatus::Supported);
    assert_eq!(store.analysis_run_count()?, 2);
    assert!(store.analysis_targets(&processor)?.is_empty());
    let id = recurring.id.clone();
    store.feedback(&id, FeedbackAction::Reject, None)?;
    assert!(store.observations(false)?.iter().all(|o| o.id != id));
    let mut new_processor = processor.clone();
    new_processor.version = "test-next-version".into();
    process(&mut store, &new_processor)?;
    let rejected = store
        .observations(true)?
        .into_iter()
        .find(|o| o.id == id)
        .unwrap();
    assert_eq!(rejected.status, ObservationStatus::UserRejected);
    assert_eq!(rejected.distinct_conversations, 2);
    assert_eq!(rejected.occurrences, 2);
    assert_eq!(rejected.feedback_count, 1);
    store.feedback(
        &id,
        FeedbackAction::Correct,
        Some("Prefer simple implementations for personal prototypes"),
    )?;
    let corrected = store
        .observations(false)?
        .into_iter()
        .find(|o| o.id == id)
        .unwrap();
    assert_eq!(corrected.status, ObservationStatus::UserConfirmed);
    assert_eq!(
        corrected.display_text(),
        "Prefer simple implementations for personal prototypes"
    );
    assert_eq!(corrected.feedback_count, 2);
    drop(store);
    assert_eq!(
        Store::open(temp.path())?
            .observations(false)?
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .display_text(),
        "Prefer simple implementations for personal prototypes"
    );
    Ok(())
}

#[test]
fn fabricated_evidence_and_stale_analysis_leave_no_completed_checkpoint() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    store.import(&sources)?;
    let processor = Processor::rules();
    let target = store.analysis_targets(&processor)?.remove(0);
    let mut output = extract_rules(&target.conversation, target.revision);
    output.observations[0].evidence[0].quote = "fabricated evidence".into();
    assert!(store.save_analysis(&target, &processor, output).is_err());
    assert_eq!(store.analysis_run_count()?, 0);
    assert!(store.observations(true)?.is_empty());
    let index = sources
        .iter()
        .position(|c| c.stable_id() == target.conversation.stable_id())
        .unwrap();
    sources[index].messages[0]
        .content
        .push_str(" changed source");
    store.import(&sources)?;
    assert!(
        store
            .save_analysis(&target, &processor, AnalysisOutput::default())
            .is_err()
    );
    assert_eq!(store.analysis_run_count()?, 0);
    Ok(())
}

#[test]
fn source_revision_updates_retire_stale_evidence_without_erasing_review_history() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    store.import(&sources)?;
    process(&mut store, &Processor::rules())?;
    let id = store
        .observations(false)?
        .into_iter()
        .find(|o| o.observation.canonical_value == "keep_implementations_simple")
        .unwrap()
        .id;
    store.feedback(&id, FeedbackAction::Confirm, None)?;
    for message in &mut sources[0].messages {
        message.content = "A different message without this instruction".into();
    }
    store.import(&sources)?;
    process(&mut store, &Processor::rules())?;
    let current = store
        .observations(false)?
        .into_iter()
        .find(|o| o.id == id)
        .unwrap();
    assert_eq!(current.distinct_conversations, 1);
    assert_eq!(current.status, ObservationStatus::UserConfirmed);
    assert_eq!(current.feedback_count, 1);
    assert_eq!(current.observation.evidence.len(), 1);
    Ok(())
}

#[test]
fn feedback_validation_does_not_modify_status_on_invalid_requests() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    store.import(&fixture()?)?;
    process(&mut store, &Processor::rules())?;
    let id = store.observations(false)?[0].id.clone();
    assert!(store.feedback(&id, FeedbackAction::Correct, None).is_err());
    assert!(
        store
            .feedback(&id, FeedbackAction::Correct, Some("  "))
            .is_err()
    );
    assert!(
        store
            .feedback(&id, FeedbackAction::Reject, Some("wrong"))
            .is_err()
    );
    assert!(
        store
            .feedback("unknown", FeedbackAction::Confirm, None)
            .is_err()
    );
    assert_eq!(store.observations(false)?[0].feedback_count, 0);
    Ok(())
}

#[test]
fn explicit_model_selection_retires_only_completed_source_results_and_keeps_review_history()
-> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    for source in &mut sources {
        source
            .messages
            .iter_mut()
            .find(|m| m.role == "user")
            .unwrap()
            .content
            .push_str(" I prefer Rust.");
    }
    store.import(&sources)?;
    let old = Processor::ollama("synthetic-old");
    let replacement = Processor::ollama("synthetic-new");
    process(&mut store, &old)?;
    let records = store.observations(false)?;
    assert!(records.len() >= 2);
    let confirmed = records[0].id.clone();
    let automatic = records[1].id.clone();
    store.feedback(&confirmed, FeedbackAction::Confirm, None)?;
    let projection = temp.path().join("model-result.md");
    std::fs::write(&projection, "Synthetic automatic finding")?;
    store.register_managed_file(
        &projection,
        &observr::model::digest(b"Synthetic automatic finding"),
        &[],
        &[automatic],
    )?;
    let mut targets = store.analysis_targets(&replacement)?;
    let first = targets.remove(0);
    store.save_analysis(&first, &replacement, AnalysisOutput::default())?;
    assert_eq!(store.select_model_results(&replacement)?, 1);
    assert!(!projection.exists());
    // Sources with no completed replacement retain their current model result.
    assert!(store.observations(false)?.iter().any(|r| r.id != confirmed));
    for target in targets {
        store.save_analysis(&target, &replacement, AnalysisOutput::default())?;
    }
    assert_eq!(store.select_model_results(&replacement)?, 1);
    let records = store.observations(false)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, confirmed);
    assert_eq!(records[0].status, ObservationStatus::UserConfirmed);
    assert_eq!(records[0].feedback_count, 1);
    assert!(store.analysis_targets(&old)?.is_empty());
    // Switching back is an explicit choice and can use this exact cached run.
    assert_eq!(store.select_model_results(&old)?, 2);
    assert!(store.observations(false)?.len() >= 2);
    let human = store
        .observations(false)?
        .into_iter()
        .find(|r| r.id == confirmed)
        .unwrap();
    assert_eq!(human.feedback_count, 1);
    assert_eq!(human.status, ObservationStatus::UserConfirmed);
    assert!(store.select_model_results(&Processor::rules()).is_err());
    Ok(())
}
