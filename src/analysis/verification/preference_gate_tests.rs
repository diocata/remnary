use super::*;
#[test]
fn tasks_questions_and_speculation_do_not_state_preferences() {
    for text in [
        "Put the draft in my local mail app.",
        "Create a new file with a puzzle game.",
        "Give me more options from 300-800 used price.",
        "How much would I net in Vienna with the same gross",
        "I bet cash would earn more than this stock package.",
        "I want another option.",
        "Do I prefer concise explanations?",
        "What if I prefer Rust?",
    ] {
        assert!(!declared_preference(text), "{text}");
    }
    for text in [
        "I prefer Rust for local tools.",
        "I don't like extra files.",
        "For work messages, I prefer concise explanations.",
        "Prefiro explicações curtas.",
        "Não gosto de respostas longas.",
        "I usually prefer a short answer.",
    ] {
        assert!(declared_preference(text), "{text}");
    }
}
#[test]
fn all_omitted_preferences_still_require_exact_citations() -> Result<()> {
    let mut source = crate::model::Conversation {
        provider: "synthetic".into(),
        source_id: "one".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: Value::Null,
        messages: vec![],
    };
    source.messages.push(crate::model::Message {
        source_id: "m".into(),
        parent_source_id: None,
        role: "user".into(),
        content: "Create a new file with a puzzle game.".into(),
        created_at: None,
        metadata: Value::Null,
    });
    let output: AnalysisOutput = serde_json::from_value(
        json!({"schema_version":1,"observations":[{"kind":"explicit_preference","inference":"explicit","subject":"user","topic":"files","context":null,"canonical_value":"prefers puzzle game files","summary":"Prefers puzzle game files.","decision_event":null,"evidence":[{"provider":"synthetic","conversation_id":source.stable_id(),"revision":1,"source_message_id":"m","quote":source.messages[0].content}]}]}),
    )?;
    assert!(cited_context(&source, &output).is_ok());
    let (filtered, count) = filter_ineligible_claims(output.clone());
    assert!(filtered.observations.is_empty());
    assert_eq!(count, 1);
    let mut invalid = output;
    invalid.observations[0].evidence[0].quote = "fabricated".into();
    assert!(cited_context(&source, &invalid).is_err());
    Ok(())
}
