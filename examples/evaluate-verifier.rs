//! Synthetic semantic acceptance check for the optional local verifier.
use anyhow::{Result, ensure};
use remnary::{
    intelligence::{
        AnalysisOutput, CandidateObservation, DecisionEventType, EvidenceRef, ExtractionKind,
        InferenceType,
    },
    model::{Conversation, Message, digest},
    ollama::model_identity,
    verification::verify_observations,
};
use serde::Deserialize;
use serde_json::json;
#[derive(Deserialize)]
struct Case {
    id: String,
    kind: ExtractionKind,
    topic: String,
    decision_event: Option<DecisionEventType>,
    quotes: Vec<String>,
    source_context: Option<String>,
    canonical_value: String,
    summary: String,
    context: Option<String>,
    expected_supported: bool,
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let endpoint = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("http://127.0.0.1:11435");
    let model = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("qwen3.5:9b-q4_K_M");
    let bytes = include_bytes!("../tests/fixtures/verifier-semantic-evaluation.json");
    let mut cases: Vec<Case> = serde_json::from_slice(bytes)?;
    if args.len() > 3 {
        for id in &args[3..] {
            ensure!(
                cases.iter().any(|case| &case.id == id),
                "unknown case ID: {id}"
            );
        }
        cases.retain(|case| args[3..].contains(&case.id));
    }
    let identity = model_identity(endpoint, model)?;
    let observations = cases
        .iter()
        .map(|case| CandidateObservation {
            kind: case.kind,
            inference: if matches!(
                case.kind,
                ExtractionKind::BehavioralPattern | ExtractionKind::InferredPreference
            ) {
                InferenceType::Inferred
            } else {
                InferenceType::Explicit
            },
            subject: "user".into(),
            topic: case.topic.clone(),
            canonical_value: case.canonical_value.clone(),
            context: case.context.clone(),
            summary: case.summary.clone(),
            decision_event: case.decision_event,
            evidence: case
                .quotes
                .iter()
                .enumerate()
                .map(|(index, quote)| EvidenceRef {
                    provider: "synthetic".into(),
                    conversation_id: digest(
                        format!("remnary-synthetic-verifier-conversation:{}", case.id).as_bytes(),
                    ),
                    revision: 1,
                    source_message_id: digest(
                        format!("remnary-synthetic-verifier-message:{}:{index}", case.id)
                            .as_bytes(),
                    ),
                    quote: quote.clone(),
                })
                .collect(),
        })
        .collect();
    let source = Conversation {
        provider: "synthetic".into(),
        source_id: "semantic-context".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: serde_json::Value::Null,
        messages: cases
            .iter()
            .flat_map(|case| {
                case.quotes.iter().enumerate().map(move |(index, quote)| {
                    let content = if index == 0 {
                        case.source_context.clone().unwrap_or_else(|| quote.clone())
                    } else {
                        quote.clone()
                    };
                    assert!(
                        content.contains(quote),
                        "synthetic quote must match its source context"
                    );
                    Message {
                        source_id: digest(
                            format!("remnary-synthetic-verifier-message:{}:{index}", case.id)
                                .as_bytes(),
                        ),
                        parent_source_id: None,
                        role: "user".into(),
                        content,
                        created_at: None,
                        metadata: serde_json::Value::Null,
                    }
                })
            })
            .collect(),
    };
    let started = std::time::Instant::now();
    let (verified, rejected) = verify_observations(
        endpoint,
        model,
        &identity,
        &source,
        AnalysisOutput {
            observations,
            ..AnalysisOutput::default()
        },
    )?;
    let results=cases.iter().map(|case|{
  let accepted=verified.observations.iter().any(|o|o.evidence[0].conversation_id==digest(format!("remnary-synthetic-verifier-conversation:{}",case.id).as_bytes()));
  json!({"case":case.id,"expected_supported":case.expected_supported,"accepted":accepted,"passed":accepted==case.expected_supported})
 }).collect::<Vec<_>>();
    let passed = results.iter().filter(|r| r["passed"] == true).count();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"synthetic_only":true,"fixture_labels_are_not_model_inputs":true,"model":model,"manifest_digest":identity.digest,"daemon_version":identity.daemon_version,"fixture_sha256":digest(bytes),"selected_cases":cases.iter().map(|case|&case.id).collect::<Vec<_>>(),"seconds":started.elapsed().as_secs_f64(),"passed":passed,"total":cases.len(),"rejected":rejected,"cases":results})
        )?
    );
    ensure!(
        passed == cases.len(),
        "semantic verifier acceptance cases failed"
    );
    Ok(())
}
