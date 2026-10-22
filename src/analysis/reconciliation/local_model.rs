use super::*;
fn settings() -> (String, String) {
    (
        std::env::var("REMNARY_TEST_OLLAMA_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:11435".into()),
        std::env::var("REMNARY_TEST_OLLAMA_MODEL").unwrap_or_else(|_| "qwen3.5:9b-q4_K_M".into()),
    )
}
#[test]
#[ignore = "requires the installed local model and runs an encrypted synthetic review lifecycle"]
fn reviewed_append_uses_historical_identity() -> Result<()> {
    use crate::{
        analysis::{AnalysisEngine, analyze},
        analysis_store::{FeedbackAction, ObservationStatus, Processor},
        intelligence::{model_output_schema, validate_model_output},
        model::Message,
        security::SecretKey,
    };
    let (endpoint, model) = settings();
    let (endpoint, model) = (endpoint.as_str(), model.as_str());
    let mut source = Conversation {
        provider: "synthetic".into(),
        source_id: "review-lifecycle".into(),
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
    let version = model_identity(endpoint, model)?;
    let preview = crate::ollama::analyze_pinned_json(
        endpoint,
        model,
        &version,
        &model_output_schema(),
        &source,
        1,
    )?;
    let preview = validate_model_output(&source, 1, serde_json::from_value(preview)?)?;
    let (preview, _) =
        crate::verification::verify_observations(endpoint, model, &version, &source, preview)?;
    let mut root = preview
        .observations
        .into_iter()
        .find(|o| o.kind == crate::intelligence::ExtractionKind::ExplicitPreference)
        .ok_or_else(|| anyhow::anyhow!("positive preference control was omitted"))?;
    let generated_id = root.stable_id();
    root.canonical_value = "user favors brevity in explanations".into();
    ensure!(
        generated_id != root.stable_id(),
        "fixture must seed a different supported canonical wording"
    );
    let root_id = root.stable_id();
    let temp = tempfile::tempdir()?;
    let key = SecretKey::generate()?;
    let mut store = Store::init_encrypted(&temp.path().join("archive"), &key)?;
    store.import(std::slice::from_ref(&source))?;
    let seed = Processor {
        engine: "synthetic-review-fixture".into(),
        version: "1".into(),
        model: "synthetic".into(),
        prompt_version: "1".into(),
    };
    let target = store.analysis_targets(&seed)?.remove(0);
    store.save_analysis(
        &target,
        &seed,
        AnalysisOutput {
            observations: vec![root],
            ..AnalysisOutput::default()
        },
    )?;
    store.feedback(&root_id, FeedbackAction::Reject, None)?;
    source.messages.push(Message {
        source_id: "reply".into(),
        parent_source_id: Some("m".into()),
        role: "assistant".into(),
        content: "Acknowledged.".into(),
        created_at: None,
        metadata: Value::Null,
    });
    store.import(std::slice::from_ref(&source))?;
    assert!(store.observations(true)?.is_empty());
    assert_eq!(catalog(&store, &source)?.len(), 1);
    let result = analyze(&mut store, AnalysisEngine::Ollama { endpoint, model })?;
    ensure!(
        result.failures.is_empty(),
        "reviewed-source model analysis remained pending: {:?}",
        result.failures
    );
    assert_eq!(result.processed, 1);
    assert_eq!(result.observations, 1);
    assert!(store.observations(false)?.is_empty());
    let reviewed = store.observations(true)?.remove(0);
    assert_eq!(reviewed.id, root_id);
    assert_eq!(reviewed.status, ObservationStatus::UserRejected);
    assert_eq!(reviewed.feedback_count, 1);
    assert!(
        reviewed
            .observation
            .evidence
            .iter()
            .all(|e| e.revision == 2)
    );
    let mut repeated = source.clone();
    repeated.source_id = "review-lifecycle-two".into();
    repeated.messages.truncate(1);
    repeated.messages[0].content = "I prefer short explanations.".into();
    store.import(std::slice::from_ref(&repeated))?;
    let result = analyze(&mut store, AnalysisEngine::Ollama { endpoint, model })?;
    ensure!(
        result.failures.is_empty(),
        "cross-source identity remained pending: {:?}",
        result.failures
    );
    assert_eq!(result.processed, 1);
    assert_eq!(result.observations, 1);
    assert!(store.observations(false)?.is_empty());
    let recurring = store.observations(true)?.remove(0);
    assert_eq!(recurring.id, root_id);
    assert_eq!(recurring.distinct_conversations, 2);
    assert_eq!(recurring.feedback_count, 1);
    assert_eq!(recurring.status, ObservationStatus::UserRejected);
    // Synthetic owner reset permits automatic support, never confirmation.
    store.feedback(&root_id, FeedbackAction::Reset, None)?;
    let recurring = store.observations(false)?.remove(0);
    assert_eq!(recurring.status, ObservationStatus::Supported);
    assert_eq!(recurring.distinct_conversations, 2);
    println!(
        "Encrypted model review lifecycle passed: append preserved rejection, two distinct sources shared the durable root, explicit synthetic reset permitted supported status without confirmation."
    );
    Ok(())
}
#[test]
#[ignore = "requires the explicitly installed local Ollama model; run separately from offline tests"]
fn semantic_identity_fixture() -> Result<()> {
    let (endpoint, model) = settings();
    let (endpoint, model) = (endpoint.as_str(), model.as_str());
    let fixture = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/identity-semantic-evaluation.json"
    ));
    let cases: Vec<Value> = serde_json::from_str(fixture)?;
    let version = model_identity(endpoint, model)?;
    let start = std::time::Instant::now();
    let ids = vec!["existing-root".to_owned()];
    let mut direction_guards = 0;
    for case in &cases {
        let data = json!({"candidate":case["candidate"],"existing":[{"existing_id":ids[0],"identity":case["existing"]}]});
        let result = decode(
            structured_json(endpoint, model, PROMPT, &data, &schema(&ids))?,
            &ids,
        )?;
        let relation = guarded_meaning(&case["candidate"], &case["existing"], result[0].relation);
        direction_guards += usize::from(relation != result[0].relation);
        let actual = match relation {
            Meaning::Equivalent => "equivalent",
            Meaning::Distinct => "distinct",
            Meaning::Uncertain => "uncertain",
        };
        assert_eq!(
            actual,
            case["expected"].as_str().unwrap(),
            "case {}",
            case["id"]
        );
    }
    println!(
        "Literal direction guards applied: {direction_guards}; fixture expectations evaluate the complete production matching policy."
    );
    ensure!(
        model_identity(endpoint, model)? == version,
        "model changed during evaluation"
    );
    println!(
        "{} synthetic identity cases passed; seconds={:.2}; fixture_sha256={}; model_digest={}; daemon={}",
        cases.len(),
        start.elapsed().as_secs_f64(),
        digest(fixture.as_bytes()),
        version.digest,
        version.daemon_version
    );
    Ok(())
}
