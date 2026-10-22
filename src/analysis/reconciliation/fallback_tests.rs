use super::*;
#[test]
fn unsupported_reuse_preserves_original_and_checks_evidence_not_only_canonical_id() -> Result<()> {
    let observation: CandidateObservation = serde_json::from_value(
        json!({"kind":"explicit_preference","inference":"explicit","subject":"user","topic":"explanations","context":null,"canonical_value":"brief explanations","summary":"Prefers brief explanations.","decision_event":null,"evidence":[{"provider":"synthetic","conversation_id":"one","revision":1,"source_message_id":"m","quote":"I prefer brief explanations."}]}),
    )?;
    let mut proposed = observation.clone();
    proposed.canonical_value = "concise explanations".into();
    proposed.summary = "Prefers concise explanations.".into();
    let original = AnalysisOutput {
        observations: vec![observation.clone()],
        ..AnalysisOutput::default()
    };
    let proposal = AnalysisOutput {
        observations: vec![proposed.clone()],
        ..AnalysisOutput::default()
    };
    let kept = retain_supported_bindings(
        original.clone(),
        proposal.clone(),
        AnalysisOutput::default(),
    )?;
    assert_eq!(kept, original);
    let mut different_evidence = proposed.clone();
    different_evidence.evidence[0].source_message_id = "other".into();
    assert_eq!(proposed.stable_id(), different_evidence.stable_id());
    let supported = AnalysisOutput {
        observations: vec![different_evidence],
        ..AnalysisOutput::default()
    };
    assert_eq!(
        retain_supported_bindings(original.clone(), proposal.clone(), supported)?,
        original
    );
    assert_eq!(
        retain_supported_bindings(original, proposal.clone(), proposal.clone())?,
        proposal
    );
    Ok(())
}
