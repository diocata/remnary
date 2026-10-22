use super::*;
use crate::{
    analysis_store::{FeedbackAction, ObservationStatus, Processor},
    intelligence::extract_rules,
    model::Message,
    security::SecretKey,
};
#[test]
fn reviewed_identity_survives_append_backup_restore_and_paraphrased_reprocessing() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let key = SecretKey::generate()?;
    let mut store = Store::init_encrypted(&temp.path().join("initial"), &key)?;
    let mut source = Conversation {
        provider: "synthetic".into(),
        source_id: "one".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: Value::Null,
        messages: vec![Message {
            source_id: "m".into(),
            parent_source_id: None,
            role: "user".into(),
            content: "I prefer concise explanations.".into(),
            created_at: None,
            metadata: Value::Null,
        }],
    };
    store.import(std::slice::from_ref(&source))?;
    let p = Processor::rules();
    let target = store.analysis_targets(&p)?.remove(0);
    store.save_analysis(&target, &p, extract_rules(&source, 1))?;
    let root = store.observations(true)?.remove(0);
    store.feedback(&root.id, FeedbackAction::Reject, None)?;
    source.messages.push(Message {
        source_id: "reply".into(),
        parent_source_id: Some("m".into()),
        role: "assistant".into(),
        content: "Acknowledged.".into(),
        created_at: None,
        metadata: Value::Null,
    });
    store.import(std::slice::from_ref(&source))?;
    assert!(
        store.observations(true)?.is_empty(),
        "stale evidence is not published as current"
    );
    let backup = temp.path().join("backup.sqlite3");
    store.backup_to(&backup, Some(&key))?;
    drop(store);
    let mut store = Store::restore_new(&temp.path().join("restored"), &backup, Some(&key))?;
    let roots = catalog(&store, &source)?;
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].id, root.id);
    let target = store.analysis_targets(&p)?.remove(0);
    assert_eq!(target.revision, 2);
    let mut candidate = extract_rules(&source, 2).observations.remove(0);
    candidate.canonical_value = "favors concise explanations".into();
    assert_ne!(candidate.stable_id(), root.id);
    let mut proof = Receipt {
        snapshot: snapshot(&store.connection)?,
        distinct: BTreeSet::new(),
    };
    apply_relations(
        &mut candidate,
        &[&roots[0]],
        vec![Relation {
            existing_id: root.id.clone(),
            relation: Meaning::Equivalent,
        }],
        &mut proof,
    )?;
    store.save_reconciled_analysis(
        &target,
        &p,
        AnalysisOutput {
            observations: vec![candidate],
            ..AnalysisOutput::default()
        },
        &proof,
    )?;
    assert!(store.observations(false)?.is_empty());
    let reviewed = store.observations(true)?.remove(0);
    assert_eq!(reviewed.id, root.id);
    assert_eq!(reviewed.status, ObservationStatus::UserRejected);
    assert_eq!(reviewed.feedback_count, 1);
    assert!(
        reviewed
            .observation
            .evidence
            .iter()
            .all(|e| e.revision == 2)
    );
    store.exclude_conversation(&source.stable_id(), true)?;
    assert!(catalog(&store, &source)?.is_empty());
    store.exclude_conversation(&source.stable_id(), false)?;
    source.messages[0].content = "I prefer detailed explanations.".into();
    store.import(std::slice::from_ref(&source))?;
    assert!(
        catalog(&store, &source)?.iter().all(|r| r.id != root.id),
        "a changed quote cannot revive old source support"
    );
    let health = store.doctor()?;
    assert!(health.database_integrity && health.foreign_keys_valid);
    Ok(())
}
