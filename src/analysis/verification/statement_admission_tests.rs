use super::*;
use crate::{
    intelligence::{ExtractionKind, extract_rules},
    model::Message,
};
fn candidate(
    content: &str,
    quote: &str,
) -> (Conversation, crate::intelligence::CandidateObservation) {
    let source = Conversation {
        provider: "synthetic".into(),
        source_id: "admission".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: Value::Null,
        messages: vec![Message {
            source_id: "m".into(),
            parent_source_id: None,
            role: "user".into(),
            content: content.into(),
            created_at: None,
            metadata: Value::Null,
        }],
    };
    let seed = Conversation {
        messages: vec![Message {
            content: "I prefer Rust.".into(),
            ..source.messages[0].clone()
        }],
        ..source.clone()
    };
    let mut observation = extract_rules(&seed, 1).observations.remove(0);
    observation.evidence[0].quote = quote.into();
    (source, observation)
}
#[test]
fn initial_selections_cannot_become_reaffirmations() {
    for quote in [
        "I chose SQLite for this prototype.",
        "I selected Tantivy for the portable recorder.",
        "I selected StillDB for the portable recorder.",
        "I chose ConfirmFlow for this prototype.",
    ] {
        assert!(decision_signal(quote));
        assert!(!reaffirmation_signal(quote));
        let mut proposed = super::tests::output();
        let claim = &mut proposed.observations[0];
        claim.kind = crate::intelligence::ExtractionKind::Decision;
        claim.decision_event = Some(crate::intelligence::DecisionEventType::Confirmation);
        claim.evidence[0].quote = quote.into();
        proposed.observations.truncate(1);
        assert!(filter_ineligible_claims(proposed).0.observations.is_empty());
    }
    for quote in [
        "I confirm my earlier choice of SQLite.",
        "I still stand by Tantivy for the portable recorder.",
        "Reafirmo a minha escolha de Rust.",
        "Mantenho a minha decisão anterior.",
    ] {
        assert!(reaffirmation_signal(quote));
        let mut proposed = super::tests::output();
        let claim = &mut proposed.observations[0];
        claim.kind = crate::intelligence::ExtractionKind::Decision;
        claim.decision_event = Some(crate::intelligence::DecisionEventType::Confirmation);
        claim.evidence[0].quote = quote.into();
        proposed.observations.truncate(1);
        assert_eq!(filter_ineligible_claims(proposed).0.observations.len(), 1);
    }
}

#[test]
fn unresolved_project_relations_need_substantive_cited_project_evidence() {
    for quote in [
        "This is separate from my website project.",
        "It is unrelated to my gardening project.",
        "That is different from our music project.",
    ] {
        assert!(unresolved_project_relation(quote));
    }
    for quote in [
        "I'm building SignalLog to index local transcripts.",
        "SignalLog is separate from my website project.",
        "This is separate from my website project. I'm building SignalLog to index local transcripts.",
    ] {
        assert!(!unresolved_project_relation(quote));
    }
    let mut output = super::tests::output();
    let mut claim = output.observations.remove(0);
    claim.kind = crate::intelligence::ExtractionKind::ProjectReference;
    claim.evidence[0].quote = "It is unrelated to my gardening project.".into();
    let mut proposed = AnalysisOutput {
        observations: vec![claim.clone()],
        ..AnalysisOutput::default()
    };
    assert!(
        filter_ineligible_claims(proposed.clone())
            .0
            .observations
            .is_empty()
    );
    let mut substantive = claim.evidence[0].clone();
    substantive.quote = "I'm building SignalLog to index local transcripts.".into();
    proposed.observations[0].evidence.push(substantive);
    assert_eq!(filter_ineligible_claims(proposed).0.observations.len(), 1);
}

#[test]
fn preference_quotes_keep_full_scope_and_do_not_promote_tentative_questions() {
    for (source, quote) in [
        ("I prefer Rust only for small prototypes.", "I prefer Rust"),
        (
            "I like the dashboard. Can it filter many items?",
            "I like the dashboard. Can it filter many items?",
        ),
        (
            "I am choosing a book; I like mountains, history and other topics.",
            "I like mountains",
        ),
        (
            "I prefer Rust, only for prototypes, contact [REDACTED].",
            "I prefer Rust",
        ),
    ] {
        let (source, observation) = candidate(source, quote);
        assert!(
            !complete_preference_quotes(&source, &observation),
            "{quote}"
        );
    }
    for (source, quote) in [
        (
            "I prefer Rust only for small prototypes.",
            "I prefer Rust only for small prototypes.",
        ),
        (
            "I prefer Rust. Another independent statement.",
            "I prefer Rust",
        ),
        ("I prefer Rust; contact [REDACTED].", "I prefer Rust"),
    ] {
        let (source, observation) = candidate(source, quote);
        assert!(complete_preference_quotes(&source, &observation), "{quote}");
    }
}
#[test]
fn routine_requests_need_actual_choice_language_before_becoming_decision_events() {
    for quote in [
        "Give me a 3 liner on my question",
        "Give me the summary in Markdown.",
        "Find my histories for these assistants.",
        "Send me the recipe website.",
    ] {
        let (_, mut observation) = candidate(quote, quote);
        observation.kind = ExtractionKind::Decision;
        observation.decision_event = Some(crate::intelligence::DecisionEventType::Consideration);
        let (filtered, omitted) = filter_ineligible_claims(AnalysisOutput {
            observations: vec![observation],
            ..AnalysisOutput::default()
        });
        assert!(filtered.observations.is_empty());
        assert_eq!(omitted, 1);
    }
    for quote in [
        "What about DuckDB plus files?",
        "I selected SQLite for this prototype.",
        "Let's try generated images before adding dependencies.",
        "I am aligning with approach B.",
        "Talvez usar ficheiros.",
    ] {
        assert!(decision_signal(quote), "{quote}");
    }
}
