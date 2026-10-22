use super::*;
use crate::intelligence::extract_rules;
use crate::model::{Conversation, Message};
pub(super) fn output() -> AnalysisOutput {
    let source = Conversation {
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
    };
    let output = extract_rules(&source, 1);
    assert_eq!(output.observations.len(), 2);
    output
}
#[test]
fn repetition_counts_relevant_messages_not_quotes_or_adjacent_requests() {
    let texts = [
        "Show recipe amounts in millilitres and cups.",
        "Which site published this recipe?",
        "For this other recipe, use millilitres and cups again.",
    ];
    let source = Conversation {
        provider: "synthetic".into(),
        source_id: "one".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: Value::Null,
        messages: texts
            .iter()
            .enumerate()
            .map(|(i, t)| Message {
                source_id: format!("m{i}"),
                parent_source_id: None,
                role: "user".into(),
                content: (*t).into(),
                created_at: None,
                metadata: Value::Null,
            })
            .collect(),
    };
    let mut observation = output().observations.remove(0);
    observation.kind = crate::intelligence::ExtractionKind::InferredPreference;
    observation.inference = crate::intelligence::InferenceType::Inferred;
    observation.topic = "communication.explanations".into();
    observation.canonical_value = "prefers recipe amounts in millilitres and cups".into();
    observation.evidence = source
        .messages
        .iter()
        .map(|m| crate::intelligence::EvidenceRef {
            provider: source.provider.clone(),
            conversation_id: source.stable_id(),
            revision: 1,
            source_message_id: m.source_id.clone(),
            quote: m.content.clone(),
        })
        .collect();
    let mut duplicated = source.clone();
    duplicated.messages.push(source.messages[0].clone());
    assert_eq!(inference_message_groups(&duplicated, &observation).len(), 3);
    let groups = inference_message_groups(&source, &observation);
    let result = json!({"schema_version":1,"verdicts":[
        {"candidate_id":0,"verdict":"supported","reason":"fully_supported"},
        {"candidate_id":1,"verdict":"unsupported","reason":"unsupported_detail"},
        {"candidate_id":2,"verdict":"supported","reason":"fully_supported"}]});
    let kept = apply_repeated_support(observation.clone(), groups, result)
        .unwrap()
        .unwrap();
    assert_eq!(kept.evidence.len(), 2);
    assert!(kept.evidence.iter().all(|e| e.source_message_id != "m1"));
    observation.evidence.truncate(1);
    let mut second_quote = observation.evidence[0].clone();
    second_quote.quote = "cups".into();
    observation.evidence.push(second_quote);
    let groups = inference_message_groups(&source, &observation);
    assert_eq!(groups.len(), 1);
    let result = json!({"schema_version":1,"verdicts":[{"candidate_id":0,"verdict":"supported","reason":"fully_supported"}]});
    assert!(
        apply_repeated_support(observation, groups, result)
            .unwrap()
            .is_none()
    );
}

#[test]
fn correction_eligibility_preserves_approval_context_and_genuine_contrasts() {
    let praise = "You renamed those documents without asking, and that was exactly what I wanted.";
    for (text, quote, expected) in [
        (praise, praise, true),
        (praise, "You renamed those documents without asking", true),
        (
            "That was exactly what I wanted. Thank you.",
            "That was exactly what I wanted. Thank you.",
            true,
        ),
        (
            "That was exactly what I wanted. You changed permissions without asking. Restore them.",
            "That was exactly what I wanted. You changed permissions without asking. Restore them.",
            false,
        ),
        (
            "That was exactly what I wanted; you changed permissions without asking.",
            "That was exactly what I wanted; you changed permissions without asking.",
            false,
        ),
        (
            "Revert the permissions you changed after I asked you to leave them alone.",
            "Revert the permissions you changed after I asked you to leave them alone.",
            false,
        ),
        (
            "You renamed the files exactly what I wanted, but you deleted my notes without asking.",
            "you deleted my notes without asking",
            false,
        ),
        (
            "That was exactly what I wanted. You changed permissions after I asked you not to.",
            "You changed permissions after I asked you not to.",
            false,
        ),
    ] {
        let source = Conversation {
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
                content: text.into(),
                created_at: None,
                metadata: Value::Null,
            }],
        };
        let mut claim = output().observations.remove(0);
        claim.kind = crate::intelligence::ExtractionKind::Correction;
        claim.evidence[0].quote = quote.into();
        assert_eq!(
            correction_has_only_approval(&source, &claim),
            expected,
            "{text}"
        );
    }
}

#[test]
fn source_context_is_required_exact_and_omits_raw_metadata() {
    let output = output();
    let mut source = Conversation {
        provider: "synthetic".into(),
        source_id: "one".into(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: json!({"private":"must-not-be-in-prompt"}),
        messages: vec![],
    };
    assert!(cited_context(&source, &output).is_err());
    source.messages.push(Message {
        source_id: "m".into(),
        parent_source_id: None,
        role: "user".into(),
        content: "I prefer Rust. I prefer SQLite.".into(),
        created_at: None,
        metadata: json!({"private":"must-not-be-in-prompt"}),
    });
    let context = cited_context(&source, &output).unwrap();
    assert_eq!(context.len(), 1);
    source.messages[0]
        .content
        .push_str(" Context-only sentinel remains outside every citation.");
    assert!(needs_context(&source, &output));
    let support = verification_input(&source, &output, false).unwrap();
    let descriptions = description_input(&output);
    for candidate in descriptions["candidates"].as_array().unwrap() {
        assert!(candidate["identity"].get("evidence").is_none());
        assert!(candidate["identity"].get("summary").is_none());
        assert!(candidate.get("description").is_some());
    }
    let context = verification_input(&source, &output, true).unwrap();
    assert!(support.get("context_messages").is_none());
    assert!(!support.to_string().contains("Context-only sentinel"));
    assert!(context.to_string().contains("Context-only sentinel"));
    let mut renamed = source.clone();
    renamed.provider = "other-provider".into();
    renamed.source_id = "misleading-supported-test-label".into();
    renamed.messages[0].source_id = "misleading-unsupported-message-label".into();
    let mut relabeled = output.clone();
    for observation in &mut relabeled.observations {
        for citation in &mut observation.evidence {
            citation.provider = renamed.provider.clone();
            citation.conversation_id = renamed.stable_id();
            citation.source_message_id = renamed.messages[0].source_id.clone();
            citation.revision = 7;
        }
    }
    assert_eq!(
        verification_input(&renamed, &relabeled, false).unwrap(),
        support
    );
    assert_eq!(
        verification_input(&renamed, &relabeled, true).unwrap(),
        context
    );
    for input in [&support, &context] {
        for candidate in input["candidates"].as_array().unwrap() {
            for citation in candidate["observation"]["evidence"].as_array().unwrap() {
                assert!(citation.get("quotes").is_some());
                for key in [
                    "provider",
                    "conversation_id",
                    "revision",
                    "source_message_id",
                ] {
                    assert!(
                        citation.get(key).is_none(),
                        "{key} must stay outside semantic verification"
                    );
                }
            }
        }
    }
    assert!(
        context["candidates"][0]["observation"]["evidence"][0]["source_context"]
            .as_str()
            .unwrap()
            .contains("Context-only sentinel")
    );
    assert!(
        !serde_json::to_string(&context)
            .unwrap()
            .contains("must-not-be-in-prompt")
    );
    source.messages[0].content = "A different message.".into();
    assert!(cited_context(&source, &output).is_err());
}

#[test]
fn semantic_quotes_preserve_message_groups_without_storage_identity() {
    let mut output = output();
    let second = output.observations[1].evidence[0].clone();
    output.observations[0].evidence.push(second);
    output.observations.truncate(1);
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
            content: "I prefer Rust. I prefer SQLite.".into(),
            created_at: None,
            metadata: Value::Null,
        }],
    };
    let data = verification_input(&source, &output, false).unwrap();
    let evidence = data["candidates"][0]["observation"]["evidence"]
        .as_array()
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0]["quotes"].as_array().unwrap().len(), 2);
    assert!(
        evidence[0]["quotes"][0]
            .as_str()
            .unwrap()
            .starts_with("I prefer Rust")
    );
    source.messages.push(Message {
        source_id: "other".into(),
        parent_source_id: None,
        role: "user".into(),
        content: output.observations[0].evidence[1].quote.clone(),
        created_at: None,
        metadata: Value::Null,
    });
    output.observations[0].evidence[1].source_message_id = "other".into();
    let data = verification_input(&source, &output, true).unwrap();
    let evidence = data["candidates"][0]["observation"]["evidence"]
        .as_array()
        .unwrap();
    assert_eq!(evidence.len(), 2);
    assert!(
        evidence
            .iter()
            .all(|e| e["quotes"].as_array().unwrap().len() == 1)
    );
    assert_eq!(
        evidence[1]["source_context"],
        output.observations[0].evidence[1].quote
    );
    assert!(
        evidence
            .iter()
            .all(|e| e.get("source_message_id").is_none())
    );
}

#[test]
fn uncertain_verdicts_never_authorize_observations() {
    let output = output();
    let result = json!({"schema_version":1,"verdicts":[
        {"candidate_id":0,"verdict":"uncertain","reason":"ambiguous_evidence"},
        {"candidate_id":1,"verdict":"unsupported","reason":"unsupported_detail"}]});
    let (accepted, omitted) = apply(output, result).unwrap();
    assert!(accepted.observations.is_empty());
    assert_eq!(omitted, 2);
}

#[test]
fn rejects_partial_duplicate_unknown_and_inconsistent_verdicts() {
    for verdicts in [
        json!([]),
        json!([{"candidate_id":0,"verdict":"supported","reason":"fully_supported"},{"candidate_id":0,"verdict":"supported","reason":"fully_supported"}]),
        json!([{"candidate_id":0,"verdict":"supported","reason":"fully_supported"},{"candidate_id":2,"verdict":"supported","reason":"fully_supported"}]),
        json!([{"candidate_id":0,"verdict":"unsupported","reason":"fully_supported"},{"candidate_id":1,"verdict":"supported","reason":"fully_supported"}]),
    ] {
        assert!(apply(output(), json!({"schema_version":1,"verdicts":verdicts})).is_err());
    }
}
#[test]
fn all_verdict_reason_pairs_obey_support_contract() {
    let branches = schema(1)["properties"]["verdicts"]["items"]["anyOf"]
        .as_array()
        .unwrap()
        .clone();
    for verdict in ["supported", "unsupported", "uncertain"] {
        for reason in [
            "fully_supported",
            "unsupported_detail",
            "overstated_scope",
            "wrong_kind_or_inference",
            "ambiguous_evidence",
        ] {
            let consistent = (verdict == "supported") == (reason == "fully_supported");
            let allowed = branches.iter().any(|branch| {
                branch["properties"]["verdict"]["enum"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(verdict))
                    && branch["properties"]["reason"]["enum"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(reason))
            });
            assert_eq!(allowed, consistent, "{verdict}/{reason}");
            let mut original = output();
            original.observations.truncate(1);
            let result = apply(
                original,
                json!({"schema_version":1,"verdicts":[{
                "candidate_id":0,"verdict":verdict,"reason":reason}]}),
            );
            assert_eq!(result.is_ok(), consistent, "{verdict}/{reason}");
            if let Ok((kept, omitted)) = result {
                assert_eq!(kept.observations.len(), usize::from(verdict == "supported"));
                assert_eq!(omitted, usize::from(verdict != "supported"));
            }
        }
    }
}

#[test]
fn abstention_filters_without_modifying_surviving_claims_or_evidence() {
    let original = output();
    let expected = serde_json::to_value(&original.observations[1]).unwrap();
    let (verified,rejected)=apply(original,json!({"schema_version":1,"verdicts":[{"candidate_id":1,"verdict":"supported","reason":"fully_supported"},{"candidate_id":0,"verdict":"uncertain","reason":"ambiguous_evidence"}]})).unwrap();
    assert_eq!(rejected, 1);
    assert_eq!(verified.observations.len(), 1);
    assert_eq!(
        serde_json::to_value(&verified.observations[0]).unwrap(),
        expected
    );
}
