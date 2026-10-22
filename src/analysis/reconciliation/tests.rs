use super::*;
use crate::{
    analysis_store::{FeedbackAction, Processor},
    intelligence::extract_rules,
    model::Message,
};
fn source() -> Conversation {
    Conversation {
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
            content: "I prefer Rust. I prefer SQLite.".into(),
            created_at: None,
            metadata: Value::Null,
        }],
    }
}
fn receipt(store: &Store) -> Receipt {
    Receipt {
        snapshot: snapshot(&store.connection).unwrap(),
        distinct: BTreeSet::new(),
    }
}
#[test]
fn reversed_transitions_cannot_reuse_a_reviewed_root() -> Result<()> {
    let mut candidate = extract_rules(&source(), 1).observations.remove(0);
    candidate.kind = crate::intelligence::ExtractionKind::Decision;
    candidate.decision_event = Some(crate::intelligence::DecisionEventType::Reversal);
    candidate.canonical_value = "changed from Meilisearch to Tantivy".into();
    candidate.summary = crate::intelligence::render_identity_summary(&candidate);
    let original_id = candidate.stable_id();
    let mut old = candidate.clone();
    old.canonical_value = "changed from Tantivy to Meilisearch".into();
    old.summary = crate::intelligence::render_identity_summary(&old);
    let root = ObservationRecord {
        id: old.stable_id(),
        observation: old,
        status: crate::analysis_store::ObservationStatus::UserRejected,
        user_value: None,
        first_seen: None,
        last_seen: None,
        distinct_conversations: 1,
        occurrences: 1,
        feedback_count: 1,
        conversation_origins: BTreeMap::new(),
        message_origins: BTreeMap::new(),
    };
    let mut proof = Receipt {
        snapshot: String::new(),
        distinct: BTreeSet::new(),
    };
    apply_relations(
        &mut candidate,
        &[&root],
        vec![Relation {
            existing_id: root.id.clone(),
            relation: Meaning::Equivalent,
        }],
        &mut proof,
    )?;
    assert_eq!(candidate.stable_id(), original_id);
    assert!(proof.permits_distinct(&original_id, &root.id));
    let unknown = json!({"kind":"decision","decision_event":"reversal","canonical_value":"abandoned Meilisearch and selected Tantivy instead"});
    assert_eq!(
        guarded_meaning(&unknown, &identity(&root.observation), Meaning::Uncertain),
        Meaning::Uncertain
    );
    Ok(())
}

#[test]
fn owner_corrections_and_deleted_keys_are_not_reinterpreted_by_model_matching() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let mut source = source();
    source.messages[0].content = "I prefer Rust.".into();
    store.import(std::slice::from_ref(&source))?;
    let p = Processor::rules();
    let target = store.analysis_targets(&p)?.remove(0);
    let original = extract_rules(&source, 1);
    store.save_analysis(&target, &p, original.clone())?;
    let root = store.observations(true)?.remove(0);
    store.feedback(
        &root.id,
        FeedbackAction::Correct,
        Some("Rust only for one experiment"),
    )?;
    let mut candidate = original.clone();
    candidate.observations[0].canonical_value = "Rust is preferred".into();
    let version = ModelIdentity {
        digest: "0".repeat(64),
        daemon_version: "synthetic".into(),
    };
    // Unreachable endpoint proves these boundaries do not request a model verdict.
    let (output, proof) = reconcile(
        &store,
        "http://127.0.0.1:9",
        "synthetic",
        &version,
        &source,
        candidate.clone(),
    )?;
    assert_eq!(output, candidate);
    let mut next = p.clone();
    next.version = "next".into();
    assert!(
        store
            .save_reconciled_analysis(&target, &next, output, &proof)
            .is_err()
    );
    assert_eq!(
        store.observations(true)?[0].display_text(),
        "Rust only for one experiment"
    );
    store.delete_observation(&root.id)?;
    let mut other = source.clone();
    other.source_id = "other".into();
    other.messages[0].content = "I prefer SQLite.".into();
    store.import(std::slice::from_ref(&other))?;
    let other_target = store.analysis_targets(&p)?.remove(0);
    store.save_analysis(&other_target, &p, extract_rules(&other, 1))?;
    let (output, _) = reconcile(
        &store,
        "http://127.0.0.1:9",
        "synthetic",
        &version,
        &source,
        original.clone(),
    )?;
    assert_eq!(output, original);
    Ok(())
}

#[test]
fn relation_contract_rejects_missing_duplicate_unknown_and_extra_fields() {
    let ids = vec!["a".into(), "b".into()];
    for bad in [
        json!({"schema_version":1,"relations":[]}),
        json!({"schema_version":1,"relations":[{"existing_id":"a","relation":"distinct"},{"existing_id":"a","relation":"distinct"}]}),
        json!({"schema_version":1,"relations":[{"existing_id":"a","relation":"distinct"},{"existing_id":"x","relation":"distinct"}]}),
        json!({"schema_version":1,"relations":[{"existing_id":"a","relation":"distinct"},{"existing_id":"b","relation":"distinct","text":"unexpected"}]}),
    ] {
        assert!(decode(bad, &ids).is_err());
    }
    assert!(decode(json!({"schema_version":1,"relations":[{"existing_id":"a","relation":"equivalent"},{"existing_id":"b","relation":"uncertain"}]}),&ids).is_ok());
}
#[test]
fn equivalent_reuses_rejected_identity_without_changing_feedback_or_quotes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let source = source();
    store.import(std::slice::from_ref(&source))?;
    let p = Processor::rules();
    let target = store.analysis_targets(&p)?.remove(0);
    let mut output = extract_rules(&source, 1);
    output.observations.truncate(1);
    store.save_analysis(&target, &p, output.clone())?;
    let root = store.observations(true)?.remove(0);
    store.feedback(&root.id, FeedbackAction::Reject, None)?;
    let mut receipt = receipt(&store);
    let mut candidate = output.observations.remove(0);
    candidate.canonical_value = "Rust is preferred".into();
    let quotes = candidate.evidence.clone();
    apply_relations(
        &mut candidate,
        &[&root],
        vec![Relation {
            existing_id: root.id.clone(),
            relation: Meaning::Equivalent,
        }],
        &mut receipt,
    )?;
    assert_eq!(candidate.stable_id(), root.id);
    assert_eq!(candidate.evidence, quotes);
    let mut next = p.clone();
    next.version = "next".into();
    store.save_reconciled_analysis(
        &target,
        &next,
        AnalysisOutput {
            observations: vec![candidate],
            ..AnalysisOutput::default()
        },
        &receipt,
    )?;
    assert!(store.observations(false)?.is_empty());
    let record = store.observations(true)?.remove(0);
    assert_eq!(record.id, root.id);
    assert_eq!(record.feedback_count, 1);
    Ok(())
}
#[test]
fn distinct_fact_in_shared_quote_survives_but_concurrent_review_invalidates_receipt() -> Result<()>
{
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let source = source();
    store.import(std::slice::from_ref(&source))?;
    let p = Processor::rules();
    let target = store.analysis_targets(&p)?.remove(0);
    let mut observations = extract_rules(&source, 1).observations;
    for o in &mut observations {
        o.evidence[0].quote = source.messages[0].content.clone();
    }
    let mut candidate = observations.remove(1);
    store.save_analysis(
        &target,
        &p,
        AnalysisOutput {
            observations,
            ..AnalysisOutput::default()
        },
    )?;
    let root = store.observations(true)?.remove(0);
    store.feedback(&root.id, FeedbackAction::Reject, None)?;
    let mut proof = receipt(&store);
    apply_relations(
        &mut candidate,
        &[&root],
        vec![Relation {
            existing_id: root.id.clone(),
            relation: Meaning::Distinct,
        }],
        &mut proof,
    )?;
    let mut next = p.clone();
    next.version = "next".into();
    let output = AnalysisOutput {
        observations: vec![candidate],
        ..AnalysisOutput::default()
    };
    store.save_reconciled_analysis(&target, &next, output.clone(), &proof)?;
    assert_eq!(store.observations(false)?.len(), 1);
    let stale = receipt(&store);
    store.feedback(&root.id, FeedbackAction::Reset, None)?;
    next.version = "after-race".into();
    assert!(
        store
            .save_reconciled_analysis(&target, &next, output, &stale)
            .unwrap_err()
            .to_string()
            .contains("changed during identity reconciliation")
    );
    assert_eq!(store.analysis_targets(&next)?.len(), 1);
    Ok(())
}
#[test]
fn uncertain_and_multiple_equivalents_never_bind_or_grant_overlap_permission() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let source = source();
    store.import(std::slice::from_ref(&source))?;
    let p = Processor::rules();
    let target = store.analysis_targets(&p)?.remove(0);
    store.save_analysis(&target, &p, extract_rules(&source, 1))?;
    let roots = store.observations(true)?;
    let mut candidate = roots[0].observation.clone();
    candidate.canonical_value = "new wording".into();
    let original = candidate.clone();
    let mut proof = receipt(&store);
    apply_relations(
        &mut candidate,
        &roots.iter().collect::<Vec<_>>(),
        roots
            .iter()
            .map(|r| Relation {
                existing_id: r.id.clone(),
                relation: Meaning::Equivalent,
            })
            .collect(),
        &mut proof,
    )?;
    assert_eq!(candidate, original);
    assert!(proof.distinct.is_empty());
    apply_relations(
        &mut candidate,
        &[&roots[0]],
        vec![Relation {
            existing_id: roots[0].id.clone(),
            relation: Meaning::Uncertain,
        }],
        &mut proof,
    )?;
    assert_eq!(candidate, original);
    assert!(proof.distinct.is_empty());
    let mut scoped = roots[0].clone();
    scoped.observation.context = Some("one specific task".into());
    scoped.id = scoped.observation.stable_id();
    apply_relations(
        &mut candidate,
        &[&scoped],
        vec![Relation {
            existing_id: scoped.id.clone(),
            relation: Meaning::Equivalent,
        }],
        &mut proof,
    )?;
    assert_eq!(
        candidate, original,
        "a model equivalence cannot add an unstated restriction"
    );
    store.delete_observation(&roots[0].id)?;
    assert!(proof.check(&store.connection).is_err());
    Ok(())
}
