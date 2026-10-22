use super::*;
use crate::{
    analysis_store::{ObservationStatus, Processor},
    intelligence::extract_rules,
    model::Message,
};
#[test]
fn paraphrase_attaches_new_evidence_to_durable_root_and_counts_real_conversations() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut store = Store::init(temp.path())?;
    let first = Conversation {
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
    let p = Processor::rules();
    store.import(std::slice::from_ref(&first))?;
    let target = store.analysis_targets(&p)?.remove(0);
    store.save_analysis(&target, &p, extract_rules(&first, 1))?;
    let root = store.observations(true)?.remove(0);
    let mut second = first.clone();
    second.source_id = "two".into();
    second.messages[0].content = "I prefer explanations that are concise.".into();
    store.import(std::slice::from_ref(&second))?;
    let target = store.analysis_targets(&p)?.remove(0);
    let mut candidate = extract_rules(&second, 1).observations.remove(0);
    let mut receipt = Receipt {
        snapshot: snapshot(&store.connection)?,
        distinct: BTreeSet::new(),
    };
    apply_relations(
        &mut candidate,
        &[&root],
        vec![Relation {
            existing_id: root.id.clone(),
            relation: Meaning::Equivalent,
        }],
        &mut receipt,
    )?;
    store.save_reconciled_analysis(
        &target,
        &p,
        AnalysisOutput {
            observations: vec![candidate],
            ..AnalysisOutput::default()
        },
        &receipt,
    )?;
    let records = store.observations(false)?;
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.id, root.id);
    assert_eq!(record.distinct_conversations, 2);
    assert_eq!(record.occurrences, 2);
    assert_eq!(record.status, ObservationStatus::Supported);
    assert_eq!(record.feedback_count, 0);
    assert_eq!(
        record
            .observation
            .evidence
            .iter()
            .map(|e| e.conversation_id.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    Ok(())
}
