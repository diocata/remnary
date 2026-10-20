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

#[test]
fn progress_reports_completed_and_failed_sources_without_exposing_payloads() -> Result<()> {
    use observr::analysis::{AnalysisEngine, AnalysisProgress, analyze_with_progress};
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    store.import(&fixture()?)?;
    let mut events = Vec::new();
    let summary = analyze_with_progress(&mut store, AnalysisEngine::Rules, |event| {
        events.push(event)
    })?;
    assert_eq!(summary.processed, 2);
    assert_eq!(events.len(), 4);
    assert!(matches!(
        events[0],
        AnalysisProgress::Conversation {
            current: 1,
            total: 2
        }
    ));
    assert!(matches!(
        events[1],
        AnalysisProgress::Finished {
            conversation: 1,
            success: true
        }
    ));
    events.clear();
    // Remote endpoints fail locally before any network request or content send.
    let error = analyze_with_progress(
        &mut store,
        AnalysisEngine::Ollama {
            endpoint: "https://example.invalid",
            model: "synthetic",
        },
        |event| events.push(event),
    )
    .unwrap_err();
    assert!(!error.to_string().is_empty());
    assert!(events.is_empty());
    assert_eq!(
        store
            .analysis_targets(&Processor::ollama("synthetic"))?
            .len(),
        2
    );
    Ok(())
}

#[test]
fn replacing_weights_or_daemon_invalidates_cached_runs() -> Result<()> {
    use observr::ollama::ModelIdentity;
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    store.import(&fixture()?)?;
    let mut identity = ModelIdentity {
        digest: "a".repeat(64),
        daemon_version: "0.40.6".into(),
    };
    let first = Processor::ollama_resolved("same-tag", &identity);
    process(&mut store, &first)?;
    assert!(store.analysis_targets(&first)?.is_empty());
    identity.digest = "b".repeat(64);
    let replaced = Processor::ollama_resolved("same-tag", &identity);
    assert_eq!(store.analysis_targets(&replaced)?.len(), 2);
    process(&mut store, &replaced)?;
    assert!(store.analysis_targets(&replaced)?.is_empty());
    identity.daemon_version = "0.40.7".into();
    let upgraded = Processor::ollama_resolved("same-tag", &identity);
    assert_eq!(store.analysis_targets(&upgraded)?.len(), 2);
    // Historical runs are still reusable by their exact provenance.
    assert!(store.analysis_targets(&first)?.is_empty());
    Ok(())
}

#[test]
fn unreconciled_paraphrases_cannot_bypass_an_existing_rejection() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    sources.truncate(1);
    sources[0].messages.retain(|message| message.role == "user");
    sources[0].messages.truncate(1);
    sources[0].messages[0].content =
        "I prefer Rust for local command-line tools. SQLite is another option.".into();
    store.import(&sources)?;
    let mut processor = Processor::ollama("synthetic");
    processor.version = "review-identity-a".into();
    let target = store.analysis_targets(&processor)?.remove(0);
    let output = extract_rules(&target.conversation, target.revision);
    assert_eq!(output.observations.len(), 1);
    store.save_analysis(&target, &processor, output.clone())?;
    let reviewed = store.observations(false)?.remove(0);
    store.feedback(&reviewed.id, FeedbackAction::Reject, None)?;
    let mut paraphrase = output;
    paraphrase.observations[0].canonical_value =
        "favor Rust when making local CLI applications".into();
    paraphrase.observations[0].summary = "User favors Rust for local CLI applications.".into();
    paraphrase.observations[0].kind = observr::intelligence::ExtractionKind::Topic;
    paraphrase.observations[0].evidence[0].quote = target.conversation.messages[0].content.clone();
    processor.version = "review-identity-b".into();
    let error = store
        .save_analysis(&target, &processor, paraphrase)
        .unwrap_err();
    assert!(error.to_string().contains("identity reconciliation"));
    assert!(store.observations(false)?.is_empty());
    assert_eq!(
        store.observations(true)?[0].status,
        ObservationStatus::UserRejected
    );
    assert_eq!(store.analysis_targets(&processor)?.len(), 1);
    Ok(())
}

#[test]
fn deleted_spans_block_paraphrases_after_restore_without_hiding_other_facts() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let key = observr::security::SecretKey::generate()?;
    let mut store = Store::init_encrypted(&temp.path().join("source"), &key)?;
    let mut sources = fixture()?;
    sources.truncate(1);
    sources[0].messages.retain(|message| message.role == "user");
    sources[0].messages.truncate(1);
    sources[0].messages[0].content = "I prefer 🦀 Rust. I prefer SQLite.".into();
    store.import(&sources)?;
    let mut processor = Processor::ollama("synthetic");
    processor.version = "deleted-span-a".into();
    let target = store.analysis_targets(&processor)?.remove(0);
    let output = extract_rules(&target.conversation, target.revision);
    assert_eq!(output.observations.len(), 2);
    store.save_analysis(&target, &processor, output)?;
    let deleted_id = store
        .observations(false)?
        .iter()
        .find(|record| record.observation.canonical_value.contains("rust"))
        .unwrap()
        .id
        .clone();
    store.delete_observation(&deleted_id)?;
    let backup = temp.path().join("backup.sqlite3");
    store.backup_to(&backup, Some(&key))?;
    let mut restored = Store::restore_new(&temp.path().join("restored"), &backup, Some(&key))?;
    // An unrelated message changes the conversation revision; the cited user
    // message stays unchanged, so its deletion provenance must still apply.
    sources[0].messages.push(observr::model::Message {
        source_id: "new-response".into(),
        parent_source_id: None,
        role: "assistant".into(),
        content: "A new response.".into(),
        created_at: None,
        metadata: Default::default(),
    });
    restored.import(&sources)?;
    processor.version = "deleted-span-b".into();
    let target = restored.analysis_targets(&processor)?.remove(0);
    assert_eq!(target.revision, 2);
    let mut output = extract_rules(&target.conversation, target.revision);
    let paraphrase = output
        .observations
        .iter_mut()
        .find(|observation| observation.canonical_value.contains("rust"))
        .unwrap();
    paraphrase.canonical_value = "Rust tooling topic".into();
    paraphrase.summary = "User discusses Rust tooling.".into();
    paraphrase.kind = observr::intelligence::ExtractionKind::Topic;
    paraphrase.evidence[0].quote = "Rust".into(); // shorter overlapping UTF-8 span
    assert_eq!(restored.save_analysis(&target, &processor, output)?, 1);
    let visible = restored.observations(false)?;
    assert_eq!(visible.len(), 1);
    assert!(visible[0].observation.canonical_value.contains("sqlite"));
    assert!(
        restored.snapshot(&sources[0].stable_id(), None)?.messages[0]
            .content
            .contains("Rust")
    );
    // New user text is a new source statement, rather than an old extraction.
    sources[0].messages[0].content = "I prefer Python. I prefer SQLite.".into();
    restored.import(&sources)?;
    let target = restored.analysis_targets(&processor)?.remove(0);
    let output = extract_rules(&target.conversation, target.revision);
    assert_eq!(restored.save_analysis(&target, &processor, output)?, 2);
    assert!(
        restored
            .observations(false)?
            .iter()
            .any(|record| record.observation.canonical_value.contains("python"))
    );
    let health = restored.doctor()?;
    assert!(health.database_integrity && health.foreign_keys_valid);
    assert_eq!(health.pending_projection_cleanup, 0);
    Ok(())
}

#[test]
fn prior_review_does_not_block_a_distinct_nonoverlapping_statement() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    sources.truncate(1);
    sources[0].messages.retain(|message| message.role == "user");
    sources[0].messages.truncate(1);
    sources[0].messages[0].content = "I prefer Rust. I prefer SQLite.".into();
    store.import(&sources)?;
    let mut processor = Processor::ollama("synthetic");
    processor.version = "nonoverlap-a".into();
    let target = store.analysis_targets(&processor)?.remove(0);
    let mut output = extract_rules(&target.conversation, target.revision);
    output
        .observations
        .retain(|observation| observation.canonical_value.contains("rust"));
    store.save_analysis(&target, &processor, output)?;
    let id = store.observations(false)?[0].id.clone();
    store.feedback(&id, FeedbackAction::Reject, None)?;
    let mut output = extract_rules(&target.conversation, target.revision);
    output
        .observations
        .retain(|observation| observation.canonical_value.contains("sqlite"));
    processor.version = "nonoverlap-b".into();
    assert_eq!(store.save_analysis(&target, &processor, output)?, 1);
    assert_eq!(store.observations(false)?.len(), 1);
    assert!(
        store.observations(false)?[0]
            .observation
            .canonical_value
            .contains("sqlite")
    );
    assert!(
        store
            .observations(true)?
            .iter()
            .any(|record| record.id == id && record.status == ObservationStatus::UserRejected)
    );
    Ok(())
}

#[test]
fn verified_reprocessing_refreshes_unreviewed_wording_and_preserves_owner_text() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut sources = fixture()?;
    sources.truncate(1);
    sources[0].messages.retain(|message| message.role == "user");
    sources[0].messages.truncate(1);
    sources[0].messages[0].content = "I prefer Rust.".into();
    store.import(&sources)?;
    let mut processor = Processor::ollama("synthetic");
    processor.version = "wording-a".into();
    let target = store.analysis_targets(&processor)?.remove(0);
    let current = extract_rules(&target.conversation, target.revision);
    let expected = current.observations[0].summary.clone();
    let mut legacy = current.clone();
    legacy.observations[0].summary = "An old automatic interpretation.".into();
    store.save_analysis(&target, &processor, legacy)?;
    let id = store.observations(false)?[0].id.clone();
    processor.version = "wording-b".into();
    store.save_analysis(&target, &processor, current.clone())?;
    assert_eq!(store.observations(false)?[0].id, id);
    assert_eq!(store.observations(false)?[0].observation.summary, expected);
    store.feedback(&id, FeedbackAction::Confirm, None)?;
    let mut revised = current.clone();
    revised.observations[0].summary = "A new automatic paraphrase.".into();
    processor.version = "wording-c".into();
    store.save_analysis(&target, &processor, revised.clone())?;
    assert_eq!(store.observations(false)?[0].observation.summary, expected);
    store.feedback(
        &id,
        FeedbackAction::Correct,
        Some("Owner's authoritative wording"),
    )?;
    processor.version = "wording-d".into();
    store.save_analysis(&target, &processor, revised.clone())?;
    assert_eq!(
        store.observations(false)?[0].display_text(),
        "Owner's authoritative wording"
    );
    store.feedback(&id, FeedbackAction::Reset, None)?;
    processor.version = "wording-e".into();
    store.save_analysis(&target, &processor, revised)?;
    assert_eq!(
        store.observations(false)?[0].observation.summary,
        "A new automatic paraphrase."
    );
    Ok(())
}
