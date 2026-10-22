use super::*;
use crate::model::Message;

fn conversation(source_id: &str, messages: &[(&str, &str)]) -> Conversation {
    Conversation {
        provider: "chatgpt".to_owned(),
        source_id: source_id.to_owned(),
        title: None,
        created_at: None,
        updated_at: None,
        metadata: Value::Null,
        messages: messages
            .iter()
            .enumerate()
            .map(|(index, (role, content))| Message {
                source_id: format!("message-{index}"),
                parent_source_id: None,
                role: (*role).to_owned(),
                content: (*content).to_owned(),
                created_at: None,
                metadata: Value::Null,
            })
            .collect(),
    }
}

fn explicit() -> (Conversation, AnalysisOutput) {
    let source = conversation("one", &[("user", "I prefer Rust.")]);
    let output = extract_rules(&source, 1);
    (source, output)
}

#[test]
fn comparison_requests_have_bounded_positive_request_forms() {
    for text in [
        "which tool is best?",
        "which option is more portable?",
        "compare rust and go.",
        "could you compare rust with go?",
        "rust or go for the api?",
    ] {
        assert!(is_comparison_request(text), "{text}");
    }
    for text in [
        "don't compare rust and go.",
        "compare nothing and do not answer.",
        "i compare rust and go.",
        "how does rust or go allocate memory?",
        "rust has a parser or compiler for source code?",
        "which option is not better?",
        "if rust or go for the api?",
        "which tool is best",
    ] {
        assert!(!is_comparison_request(text), "{text}");
    }
}

#[test]
fn comparison_pattern_requires_different_messages_and_question_text() -> Result<()> {
    let single = conversation(
        "one",
        &[("user", "Which tool is best? Which option is more portable?")],
    );
    assert!(comparison_requests(&single, 1).is_none());
    let repeated = conversation(
        "one",
        &[
            ("user", "Which tool is best?"),
            ("user", "WHICH  tool is best ?"),
        ],
    );
    assert!(comparison_requests(&repeated, 1).is_none());
    let source = conversation(
        "one",
        &[
            ("user", "Which tool is best?"),
            ("user", "Rust or Go for the API?"),
        ],
    );
    let pattern = comparison_requests(&source, 1).unwrap();
    assert_eq!(pattern.kind, ExtractionKind::BehavioralPattern);
    assert_eq!(pattern.inference, InferenceType::Inferred);
    assert_eq!(pattern.evidence.len(), 2);
    let output = validate_output(
        &source,
        1,
        AnalysisOutput {
            observations: vec![pattern],
            ..Default::default()
        },
    )?;
    assert!(output.observations[0].evidence.iter().all(|e| {
        source
            .messages
            .iter()
            .any(|m| m.source_id == e.source_message_id && m.content.contains(&e.quote))
    }));
    Ok(())
}

#[test]
fn comparison_patterns_exclude_nonowner_and_quoted_questions() {
    let source = conversation(
        "one",
        &[
            ("assistant", "Which tool is best?"),
            ("system", "Compare Rust and Go."),
            ("truncated_user", "Which option is more portable?"),
            (
                "user",
                "```text\nCompare Rust and Go.\nWhich tool is best?\n```",
            ),
            ("user", "> Which tool is best?\n> Rust or Go for the API?"),
            (
                "user",
                "Here is the transcript:\nWhich tool is best?\nRust or Go for the API?",
            ),
            (
                "user",
                "<environment_context>\nWhich tool is best?\nRust or Go for the API?\n</environment_context>",
            ),
        ],
    );
    assert!(comparison_requests(&source, 1).is_none());
    assert!(extract_rules(&source, 1).observations.is_empty());
}

#[test]
fn exact_instruction_prefixes_preserve_owner_additions_and_source_bytes() {
    let prefix = "Provider initialization task.\n";
    let hash = digest(prefix.as_bytes());
    let known = [(prefix.len(), hash.as_str())];
    assert!(strip_instruction_prefix(prefix, &known).is_empty());
    let combined = format!("{prefix}\nI prefer Rust for local tools.");
    let owner = strip_instruction_prefix(&combined, &known);
    assert_eq!(owner, "\nI prefer Rust for local tools.");
    assert!(combined.contains(owner));
    let personal = "I want to discuss steering rules and code conventions.";
    assert_eq!(strip_instruction_prefix(personal, &known), personal);
    let modified = prefix.replace("initialization", "discussion");
    assert_eq!(strip_instruction_prefix(&modified, &known), modified);
}

#[test]
fn included_rule_envelopes_preserve_only_original_owner_tail() {
    let guidance = "Synthetic automatically supplied guidance.";
    let hash = digest(guidance.as_bytes());
    let block = |name: &str| {
        format!(
            "## Included Rules ({name}) [Workspace]\n\n  {guidance}\n\n<user-rule id={name}>\n```\nI prefer generated rule content.\n```\n</user-rule>"
        )
    };
    let one = block("one.md");
    assert!(strip_included_rules(&one, &hash).is_empty());
    let tail = "\n\nI prefer Rust for local tools.";
    let combined = format!("{}\n\n{}{tail}", one, block("two.md"));
    assert_eq!(strip_included_rules(&combined, &hash), tail);
    assert!(combined.contains(strip_included_rules(&combined, &hash)));
    let crlf = combined.replace('\n', "\r\n");
    assert_eq!(
        strip_included_rules(&crlf, &hash),
        tail.replace('\n', "\r\n")
    );
    let ordinary = "I prefer flat code organization and want to discuss user-rule tags.";
    assert_eq!(strip_included_rules(ordinary, &hash), ordinary);
    let changed = one.replace(guidance, "Owner-authored notes about rule design.");
    assert_eq!(strip_included_rules(&changed, &hash), changed);
    let unclosed = one.replace("</user-rule>", "");
    assert!(strip_included_rules(&unclosed, &hash).is_empty());
    let wrong_id = one.replace("id=one.md", "id=other.md");
    assert!(strip_included_rules(&wrong_id, &hash).is_empty());
}

#[test]
fn literal_feedback_proposals_keep_complete_source_sentences() {
    for text in [
        "Revert the permissions you changed after I asked you to leave them alone.",
        "Remove PaperEdit from my computer—you installed it without asking.",
    ] {
        let source = conversation("feedback", &[("user", text)]);
        let output = propose_literal_feedback(&source, 1, AnalysisOutput::default());
        let output = validate_model_output(&source, 1, output).unwrap();
        assert_eq!(output.observations.len(), 1);
        assert_eq!(output.observations[0].canonical_value, text);
        assert_eq!(output.observations[0].evidence[0].quote, text);
        assert_eq!(output.observations[0].context, None);
        assert_eq!(output.observations[0].kind, ExtractionKind::Correction);
    }
}

#[test]
fn literal_feedback_proposals_are_bounded_and_do_not_duplicate_corrections() {
    for (role, text) in [
        ("user", "Revert the permissions on this scratch folder."),
        (
            "user",
            "If you changed permissions after I asked you not to, what would happen?",
        ),
        (
            "assistant",
            "You changed permissions after I asked you not to.",
        ),
        (
            "user",
            "Here is the transcript:\nAssistant:\nYou changed permissions after I asked you not to.",
        ),
    ] {
        let source = conversation("feedback", &[(role, text)]);
        assert!(
            propose_literal_feedback(&source, 1, AnalysisOutput::default())
                .observations
                .is_empty()
        );
    }
    let source = conversation(
        "feedback",
        &[("user", "You changed permissions after I asked you not to.")],
    );
    let once = propose_literal_feedback(&source, 1, AnalysisOutput::default());
    let twice = propose_literal_feedback(&source, 1, once.clone());
    assert_eq!(once, twice);
    let mut full = once;
    full.observations.resize(8, full.observations[0].clone());
    assert_eq!(
        propose_literal_feedback(&source, 1, full)
            .observations
            .len(),
        8
    );
}

#[test]
fn fresh_scopes_require_the_candidates_own_citations() {
    let (source, mut output) = explicit();
    output.observations[0].context = Some("for another project".into());
    // General provenance validation still accepts legacy paraphrased scopes.
    assert!(validate_output(&source, 1, output.clone()).is_ok());
    assert_eq!(retain_cited_model_scopes(&mut output), 1);
    assert!(output.observations.is_empty());
    let (_, mut output) = explicit();
    output.observations[0].context = Some("RUST".into());
    assert_eq!(retain_cited_model_scopes(&mut output), 0);
    assert_eq!(output.observations.len(), 1);
}

#[test]
fn generated_descriptions_add_no_independent_model_facts() {
    let (_, output) = explicit();
    let mut observation = output.observations[0].clone();
    observation.kind = ExtractionKind::ExplicitPreference;
    observation.canonical_value = "Rust".into();
    observation.context = Some("for local CLI tools".into());
    observation.summary = "Untrusted extra claim about Python.".into();
    assert_eq!(
        render_identity_summary(&observation),
        "Stated preference: Rust (for local CLI tools)"
    );
    observation.kind = ExtractionKind::Decision;
    observation.decision_event = Some(DecisionEventType::Consideration);
    observation.canonical_value = "DuckDB plus JSON files".into();
    observation.context = Some("for this prototype".into());
    assert_eq!(
        render_identity_summary(&observation),
        "Consideration: DuckDB plus JSON files (for this prototype)"
    );
    observation.kind = ExtractionKind::BehavioralPattern;
    observation.inference = InferenceType::Inferred;
    observation.decision_event = None;
    observation.canonical_value = "a checklist-and-tests release routine".into();
    observation.context = None;
    assert_eq!(
        render_identity_summary(&observation),
        "Possible pattern: a checklist-and-tests release routine"
    );
}

#[test]
fn model_subject_identifies_owner_instead_of_embedding_a_claim() {
    let (source, output) = explicit();
    assert!(validate_model_output(&source, 1, output.clone()).is_ok());
    for subject in [
        "database selection",
        "User prefers Rust language for local CLI tools",
    ] {
        let mut invalid = output.clone();
        invalid.observations[0].subject = subject.into();
        assert!(validate_model_output(&source, 1, invalid).is_err());
    }
}

#[test]
fn rules_skip_redacted_spans_without_turning_placeholders_into_evidence() {
    let source = conversation(
        "contact",
        &[("user", "I prefer Rust; contact person@example.test.")],
    );
    let redacted = crate::privacy::redact_conversation(&source).unwrap();
    let output = extract_rules(&redacted, 1);
    assert!(
        output
            .observations
            .iter()
            .flat_map(|o| &o.evidence)
            .all(|e| source.messages[0].content.contains(&e.quote)
                && !e.quote.contains(crate::privacy::REDACTED))
    );
    assert!(source.messages[0].content.contains("person@example.test"));
}

#[test]
fn rules_keep_exact_unicode_quotes_and_ignore_other_roles() {
    let source = conversation(
        "one",
        &[
            ("assistant", "I prefer Python."),
            ("tool", "I prefer JavaScript."),
            (
                "user",
                "I prefer café terraces. Keep implementations simple!",
            ),
        ],
    );
    let output = validate_output(&source, 3, extract_rules(&source, 3)).unwrap();
    assert_eq!(output.observations.len(), 2);
    assert!(output.observations.iter().any(|candidate| {
        candidate.evidence[0].quote == "I prefer café terraces."
            && candidate.inference == InferenceType::Explicit
    }));
    assert!(output.observations.iter().all(|candidate| {
        candidate.evidence[0].source_message_id == "message-2"
            && candidate.evidence[0].revision == 3
    }));
}

#[test]
fn source_provenance_rejects_fabrication_and_wrong_revisions() {
    let (source, valid) = explicit();
    for mutation in 0..5 {
        let mut invalid = valid.clone();
        let evidence = &mut invalid.observations[0].evidence[0];
        match mutation {
            0 => evidence.quote = "I prefer Python.".to_owned(),
            1 => evidence.source_message_id = "invented".to_owned(),
            2 => evidence.revision = 2,
            3 => evidence.provider = "claude".to_owned(),
            4 => evidence.conversation_id = "invented".to_owned(),
            _ => unreachable!(),
        }
        assert!(validate_output(&source, 1, invalid).is_err());
    }
}

#[test]
fn exploratory_option_questions_support_consideration_but_not_preference() {
    let (mut source, mut output) = explicit();
    source.messages[0].content = "What about DuckDB plus JSON files?".into();
    output.observations[0].canonical_value = "consider_duckdb_json".into();
    output.observations[0].summary = "Asked about DuckDB with JSON files.".into();
    output.observations[0].evidence[0].quote = source.messages[0].content.clone();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    output.observations[0].kind = ExtractionKind::Correction;
    assert!(validate_output(&source, 1, output.clone()).is_err());
    output.observations[0].kind = ExtractionKind::Decision;
    output.observations[0].decision_event = Some(DecisionEventType::Consideration);
    assert!(validate_model_output(&source, 1, output).is_ok());
    let (source, mut output) = explicit();
    output.observations[0].kind = ExtractionKind::Topic;
    assert!(validate_output(&source, 1, output.clone()).is_ok());
    assert!(validate_model_output(&source, 1, output).is_ok());
}

#[test]
fn mixed_questions_keep_real_preferences_and_rhetorical_corrections() {
    let (mut source, mut output) = explicit();
    source.messages[0].content = "What about DuckDB? I prefer Rust for this CLI.".into();
    output.observations[0].evidence[0].quote = "I prefer Rust for this CLI.".into();
    assert!(validate_output(&source, 1, output.clone()).is_ok());
    output.observations[0].evidence[0].quote = "DuckDB".into();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    source.messages[0].content = "Why do you keep adding unwanted dependencies?".into();
    output.observations[0].evidence[0].quote = source.messages[0].content.clone();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    output.observations[0].kind = ExtractionKind::Correction;
    assert!(validate_output(&source, 1, output).is_ok());
}

#[test]
fn shortened_quotes_cannot_turn_an_option_question_into_a_statement() {
    let (mut source, mut output) = explicit();
    source.messages[0].content = "What about DuckDB plus JSON files?".into();
    output.observations[0].evidence[0].quote = "DuckDB plus JSON files".into();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    output.observations[0].kind = ExtractionKind::Correction;
    assert!(validate_output(&source, 1, output).is_err());
    let (mut source, mut output) = explicit();
    source.messages[0].content = "Here is the transcript:\nAssistant: I prefer Rust.".into();
    output.observations[0].evidence[0].quote = "I prefer Rust.".into();
    assert!(validate_output(&source, 1, output).is_err());
}

#[test]
fn structured_question_replies_cite_only_user_answers_and_preserve_raw_escapes() {
    let (mut source, mut output) = explicit();
    source.messages[0].content = r#"<send_user_message_question_reply>
[{"question":"I prefer Python.","answer":"I prefer Rust."},{"question":"I prefer Ruby.","answer":"Keep code simple."}]
</send_user_message_question_reply>"#.into();
    let spans = personal_evidence_spans(&source.messages[0]);
    assert_eq!(spans, vec!["I prefer Rust.", "Keep code simple."]);
    output.observations[0].evidence[0].quote = "I prefer Python.".into();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    output.observations[0].evidence[0].quote = "I prefer Rust.".into();
    assert!(validate_output(&source, 1, output).is_ok());
    let rules = extract_rules(&source, 1);
    assert_eq!(rules.observations.len(), 2);
    assert!(
        rules
            .observations
            .iter()
            .all(|o| !o.summary.contains("Python") && !o.summary.contains("Ruby"))
    );
    source.messages[0].content = r#"<send_user_message_question_reply>[{"answer":"I prefer \"small\" tools.\nNo extras."}]</send_user_message_question_reply>"#.into();
    let spans = personal_evidence_spans(&source.messages[0]);
    assert_eq!(spans.len(), 1);
    assert!(spans[0].contains(r#"\"small\""#));
    assert!(source.messages[0].content.contains(spans[0]));
}

#[test]
fn provider_control_frames_are_not_personal_evidence_even_with_a_user_role() {
    let (mut source, mut output) = explicit();
    for frame in [
        "<environment_context>",
        "<codex_internal_context source=\"goal\">",
        "<permissions instructions>",
        "<app-context>",
        "# AGENTS.md instructions for /synthetic/project",
    ] {
        source.messages[0].content = format!("{frame}\nI prefer invented provider instructions.");
        output.observations[0].evidence[0].quote =
            "I prefer invented provider instructions.".into();
        assert!(!eligible_personal_evidence(&source.messages[0]));
        assert!(extract_rules(&source, 1).observations.is_empty());
        assert!(validate_output(&source, 1, output.clone()).is_err());
    }
    source.messages[0].content = "My project has an AGENTS.md file. I prefer Rust.".into();
    assert!(eligible_personal_evidence(&source.messages[0]));
    assert_eq!(extract_rules(&source, 1).observations.len(), 1);
}

#[test]
fn malformed_or_nontext_question_answers_never_supply_personal_evidence() {
    let (mut source, _) = explicit();
    for payload in [
        "broken",
        "[{\"question\":\"I prefer Ruby.\"}]",
        "[{\"answer\":true}]",
        "[{\"answer\":\"\"}]",
        "[{\"answer\":\"Here is the transcript:\\nAssistant: I prefer Ruby.\"}]",
    ] {
        source.messages[0].content = format!(
            "<send_user_message_question_reply>{payload}</send_user_message_question_reply>"
        );
        assert!(personal_evidence_spans(&source.messages[0]).is_empty());
        assert!(extract_rules(&source, 1).observations.is_empty());
    }
    source.messages[0].content =
        "<send_user_message_question_reply>[{\"answer\":\"I prefer Ruby.\"}]".into();
    assert!(!eligible_personal_evidence(&source.messages[0]));
}

#[test]
fn assistant_statements_cannot_become_user_facts() {
    let (mut source, output) = explicit();
    source.messages[0].role = "assistant".to_owned();
    assert!(validate_output(&source, 1, output).is_err());
    assert!(extract_rules(&source, 1).observations.is_empty());
}

#[test]
fn changing_subject_or_kind_cannot_launder_assistant_user_claims() {
    let (mut source, mut output) = explicit();
    source.messages[0].role = "assistant".to_owned();
    output.observations[0].kind = ExtractionKind::Topic;
    output.observations[0].subject = "person".to_owned();
    assert!(validate_output(&source, 1, output).is_err());
}

#[test]
fn rules_skip_quoted_examples_code_blocks_and_questions() {
    let source = conversation(
        "one",
        &[(
            "user",
            "\"I prefer Python.\"\n> I prefer Ruby.\n```text\nI prefer Java.\n```\nExample: I prefer Go.\nI prefer Rust?\n- I prefer SQLite.",
        )],
    );
    let output = extract_rules(&source, 1);
    assert_eq!(output.observations.len(), 1);
    assert_eq!(output.observations[0].canonical_value, "prefer sqlite");
}

#[test]
fn pasted_transcript_introductions_do_not_turn_quoted_requests_into_preferences() {
    for introduction in [
        "this was it:",
        "Here is the transcript:",
        "Here's the conversation:",
        "Example conversation:",
    ] {
        let source = conversation(
            "one",
            &[(
                "user",
                &format!(
                    "{introduction}\nAssistant\nKeep going in Claude Code.\nI prefer invented transcript facts."
                ),
            )],
        );
        assert!(extract_rules(&source, 1).observations.is_empty());
    }
    let source = conversation("one", &[("user", "I prefer Rust. Keep code simple.")]);
    assert_eq!(extract_rules(&source, 1).observations.len(), 2);
}

#[test]
fn rules_preserve_dotted_names_versions_and_negation() {
    let source = conversation(
        "one",
        &[(
            "user",
            "I prefer Node.js. I prefer Python 3.12. I’ve decided not to use DuckDB.",
        )],
    );
    let output = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
    let values: BTreeSet<_> = output
        .observations
        .iter()
        .map(|candidate| candidate.canonical_value.as_str())
        .collect();
    assert_eq!(
        values,
        BTreeSet::from(["prefer node.js", "prefer python 3.12", "not to use duckdb"])
    );
}

#[test]
fn repetitions_count_distinct_conversations_and_unique_spans() {
    let first = conversation(
        "first",
        &[
            ("user", "Keep implementations simple."),
            ("user", "Keep the code simple."),
        ],
    );
    let second = conversation("second", &[("user", "Keep code simple!")]);
    let mut observations = extract_rules(&first, 1).observations;
    observations.extend(extract_rules(&first, 1).observations);
    observations.extend(extract_rules(&second, 1).observations);
    let grouped = group_observations(&observations);
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0].occurrences, 3);
    assert_eq!(grouped[0].distinct_conversations, 2);
    assert_eq!(grouped[0].providers, ["chatgpt"]);
    observations.reverse();
    assert_eq!(grouped, group_observations(&observations));
}

#[test]
fn revisions_do_not_inflate_conversation_counts() {
    let (source, mut output) = explicit();
    output
        .observations
        .extend(extract_rules(&source, 2).observations);
    let grouped = group_observations(&output.observations);
    assert_eq!(grouped[0].distinct_conversations, 1);
    assert_eq!(grouped[0].occurrences, 2);
}

#[test]
fn contradictions_context_and_inference_are_different_identities() {
    let source = conversation(
        "one",
        &[(
            "user",
            "I prefer concise explanations for status reports.\nI prefer detailed explanations for learning.\nI don't like concise explanations.\nKeep implementations simple.\nKeep implementations simple for prototypes.",
        )],
    );
    let mut output = extract_rules(&source, 1);
    assert_eq!(output.observations.len(), 5);
    let mut inferred = output.observations[0].clone();
    inferred.inference = InferenceType::Inferred;
    inferred.kind = ExtractionKind::InferredPreference;
    let explicit_id = output.observations[0].stable_id();
    assert_ne!(explicit_id, inferred.stable_id());
    inferred.context = Some("project alpha".to_owned());
    output.observations.push(inferred);
    assert_eq!(group_observations(&output.observations).len(), 6);
}

#[test]
fn decision_events_preserve_consideration_and_reversal() {
    let source = conversation(
        "one",
        &[(
            "user",
            "I propose SQLite. I am considering DuckDB. I chose SQLite. I am reconsidering SQLite. I changed my mind about SQLite. I confirm SQLite.",
        )],
    );
    let output = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
    let events: BTreeSet<_> = output
        .observations
        .iter()
        .map(|candidate| candidate.decision_event.unwrap())
        .collect();
    assert_eq!(events.len(), 6);
    assert!(events.contains(&DecisionEventType::Reversal));
    assert!(events.contains(&DecisionEventType::Consideration));
    assert!(events.contains(&DecisionEventType::Decision));
}

#[test]
fn inferred_preferences_need_distinct_messages_not_repeated_quotes() {
    let (mut source, mut output) = explicit();
    let observation = &mut output.observations[0];
    observation.kind = ExtractionKind::InferredPreference;
    observation.inference = InferenceType::Inferred;
    observation.topic = "technology".into();
    assert!(validate_output(&source, 1, output.clone()).is_err());
    let duplicate = output.observations[0].evidence[0].clone();
    output.observations[0].evidence.push(duplicate);
    assert!(validate_output(&source, 1, output.clone()).is_err());
    let mut second = source.messages[0].clone();
    second.source_id = "another".into();
    source.messages.push(second);
    output.observations[0].evidence[1].source_message_id = "another".into();
    assert!(validate_output(&source, 1, output).is_ok());
}

#[test]
fn validation_rejects_explicit_inference_mismatch_and_sensitive_inference() {
    let (source, output) = explicit();
    let mut mismatch = output.clone();
    mismatch.observations[0].inference = InferenceType::Inferred;
    assert!(validate_output(&source, 1, mismatch).is_err());
    for topic in ["health", "workflow"] {
        let mut sensitive = output.clone();
        let observation = &mut sensitive.observations[0];
        observation.kind = ExtractionKind::BehavioralPattern;
        observation.inference = InferenceType::Inferred;
        observation.topic = topic.to_owned();
        observation.canonical_value = "user has depression".to_owned();
        observation.summary = "Possible diagnosis".to_owned();
        assert!(validate_output(&source, 1, sensitive).is_err());
    }
}

#[test]
fn malformed_fields_empty_evidence_and_unknown_fields_are_rejected() {
    let (source, output) = explicit();
    let mut empty = output.clone();
    empty.observations[0].evidence.clear();
    assert!(validate_output(&source, 1, empty).is_err());
    let mut empty = output.clone();
    empty.observations[0].summary = " ".to_owned();
    assert!(validate_output(&source, 1, empty).is_err());
    let mut malformed = serde_json::to_value(output).unwrap();
    let mut missing = malformed.clone();
    missing["observations"][0]
        .as_object_mut()
        .unwrap()
        .remove("context");
    assert!(serde_json::from_value::<AnalysisOutput>(missing).is_err());
    malformed["observations"][0]["confidence"] = json!(0.99);
    assert!(serde_json::from_value::<AnalysisOutput>(malformed).is_err());
}

#[test]
fn stable_identity_ignores_summary_and_evidence_but_preserves_scope() {
    let (_, output) = explicit();
    let candidate = &output.observations[0];
    let mut paraphrased = candidate.clone();
    paraphrased.summary = "The user explicitly prefers Rust.".to_owned();
    paraphrased.evidence.clear();
    assert_eq!(candidate.stable_id(), paraphrased.stable_id());
    paraphrased.context = Some("embedded software".to_owned());
    assert_ne!(candidate.stable_id(), paraphrased.stable_id());
}

#[test]
fn rules_skip_overlong_statements_and_bound_large_outputs() {
    let mut source = conversation("one", &[("user", "I prefer Rust.")]);
    source.messages[0].content = format!("I prefer {}. I prefer SQLite.", "a".repeat(5000));
    let output = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
    assert_eq!(output.observations.len(), 1);
    assert_eq!(output.observations[0].canonical_value, "prefer sqlite");
    source.messages[0].content = (0..1100)
        .map(|index| format!("I prefer option {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let output = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
    assert_eq!(output.observations.len(), MAX_OBSERVATIONS);
    source.messages = (0..1100)
        .map(|index| Message {
            source_id: format!("message-{index}"),
            parent_source_id: None,
            role: "user".to_owned(),
            content: "Keep implementations simple.".to_owned(),
            created_at: None,
            metadata: Value::Null,
        })
        .collect();
    let output = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
    assert_eq!(output.observations.len(), 1);
    assert_eq!(output.observations[0].evidence.len(), 1000);
}

#[test]
fn hand_labeled_synthetic_evaluation_matches_expected_groups() {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/intelligence-evaluation.json"
    )))
    .unwrap();
    let mut observations = Vec::new();
    for record in fixture["conversations"].as_array().unwrap() {
        let messages: Vec<_> = record["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| {
                (
                    message["role"].as_str().unwrap(),
                    message["content"].as_str().unwrap(),
                )
            })
            .collect();
        let mut source = conversation(record["source_id"].as_str().unwrap(), &messages);
        source.provider = record["provider"].as_str().unwrap().to_owned();
        let validated = validate_output(&source, 1, extract_rules(&source, 1)).unwrap();
        observations.extend(validated.observations);
    }
    let groups = group_observations(&observations);
    let expected = fixture["expected_groups"].as_array().unwrap();
    assert_eq!(groups.len(), expected.len());
    for expected in expected {
        let matches: Vec<_> = groups
            .iter()
            .filter(|group| {
                let actual = &group.observation;
                serde_json::to_value(actual.kind).unwrap() == expected["kind"]
                    && serde_json::to_value(actual.inference).unwrap() == expected["inference"]
                    && actual.canonical_value == expected["canonical_value"].as_str().unwrap()
                    && serde_json::to_value(actual.decision_event).unwrap()
                        == expected["decision_event"]
            })
            .collect();
        assert_eq!(matches.len(), 1, "missing or duplicate expected {expected}");
        assert_eq!(
            matches[0].occurrences,
            expected["occurrences"].as_u64().unwrap() as usize
        );
        assert_eq!(
            matches[0].distinct_conversations,
            expected["distinct_conversations"].as_u64().unwrap() as usize
        );
    }
    assert!(groups.iter().all(|group| {
        group.observation.inference == InferenceType::Explicit
            && group
                .observation
                .evidence
                .iter()
                .all(|source| !source.quote.contains("MongoDB") && !source.quote.contains("Redis"))
    }));
}
