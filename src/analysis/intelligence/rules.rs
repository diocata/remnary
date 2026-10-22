use super::{
    AnalysisOutput, CandidateObservation, DecisionEventType, EvidenceRef, ExtractionKind,
    InferenceType, MAX_OBSERVATIONS, check_text, group_observations, render_identity_summary,
};
use crate::model::{Conversation, Message, digest};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Extract direct statements and observed comparison requests without a model.
/// Quotes are retained
/// verbatim; normalized values are used only for conservative grouping. Overlong
/// statements are skipped. Output is bounded to 1,000 claim identities and
/// 1,000 source spans per claim, in deterministic identity/source order.
pub fn extract_rules(conversation: &Conversation, revision: i64) -> AnalysisOutput {
    let mut output = AnalysisOutput::default();
    for message in &conversation.messages {
        if !eligible_personal_evidence(message) {
            continue;
        }
        for quote in personal_evidence_spans(message)
            .into_iter()
            .flat_map(direct_statements)
        {
            if quote.contains(crate::privacy::REDACTED) {
                continue;
            }
            if let Some(mut observation) = classify_statement(quote) {
                if check_text(&observation.canonical_value, "canonical_value", 4096).is_err()
                    || check_text(&observation.summary, "summary", 4096).is_err()
                    || check_text(quote, "quote", 16_384).is_err()
                    || check_text(&message.source_id, "message ID", 4096).is_err()
                    || check_text(&conversation.provider, "provider", 512).is_err()
                {
                    continue;
                }
                observation
                    .evidence
                    .push(evidence_for(conversation, revision, message, quote));
                output.observations.push(observation);
            }
        }
    }
    if let Some(pattern) = comparison_requests(conversation, revision) {
        output.observations.push(pattern);
    }
    // The same claim can appear many times, but each source span is retained.
    output.observations = group_observations(&output.observations)
        .into_iter()
        .take(MAX_OBSERVATIONS)
        .map(|group| {
            let mut observation = group.observation;
            observation.evidence.truncate(1000);
            observation
        })
        .collect();
    output
}

pub(super) fn evidence_for(
    conversation: &Conversation,
    revision: i64,
    message: &Message,
    quote: &str,
) -> EvidenceRef {
    EvidenceRef {
        provider: conversation.provider.clone(),
        conversation_id: conversation.stable_id(),
        revision,
        source_message_id: message.source_id.clone(),
        quote: quote.to_owned(),
    }
}

pub(crate) fn eligible_personal_evidence(message: &Message) -> bool {
    !personal_evidence_spans(message).is_empty()
}

// Exact observed task-template prefixes, not a generic steering-word filter.
// Hashes avoid retaining provider prompt text; owner additions remain eligible.
const KIRO_STEERING_PREFIXES: &[(usize, &str)] = &[
    (
        863,
        "85c9157a9512d1ea49ceaff9cbbc4d557bad59b6d92abf97a8652a761604bc0d",
    ),
    (
        876,
        "ce4786f92c8727097e89ac5f687c1f3b798759dffcc76c76b19a0994e88d6c0e",
    ),
];
pub(super) fn strip_instruction_prefix<'a>(text: &'a str, prefixes: &[(usize, &str)]) -> &'a str {
    let candidate = text.trim_start();
    for &(length, hash) in prefixes {
        if let Some(prefix) = candidate.get(..length)
            && digest(prefix.as_bytes()) == hash
        {
            return &candidate[length..];
        }
    }
    text
}

// The observed Kiro Included Rules envelope is automatically supplied context,
// despite its native human role. Match its normalized boilerplate fingerprint
// and balanced rule envelope, preserving only the original-byte owner tail.
const KIRO_INCLUDED_RULE_GUIDANCE: &str =
    "6f0f29f3b67341a83dc17dda22ee6fbe4e98c970c826339f07b249e19821219c";
pub(super) fn strip_included_rules<'a>(mut text: &'a str, guidance_hash: &str) -> &'a str {
    loop {
        let candidate = text.trim_start();
        let Some((header, body)) = candidate.split_once('\n') else {
            return text;
        };
        let Some(name) = header
            .trim_end_matches('\r')
            .strip_prefix("## Included Rules (")
            .and_then(|s| s.strip_suffix(") [Workspace]"))
        else {
            return text;
        };
        if name.is_empty() || name.len() > 1024 || name.contains(['<', '>', '\r']) {
            return text;
        }
        let Some(open_at) = body.find("<user-rule id=") else {
            return text;
        };
        let guidance = body[..open_at]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if digest(guidance.as_bytes()) != guidance_hash {
            return text;
        }
        let opening = format!("<user-rule id={name}>");
        let Some(rule) = body[open_at..].strip_prefix(&opening) else {
            // A recognized machine envelope with no reliable owner boundary.
            return "";
        };
        let Some((_, tail)) = rule.split_once("</user-rule>") else {
            return "";
        };
        text = tail;
    }
}

/// UI question replies contain an assistant-authored question beside the user's
/// answer. Only the raw JSON string interiors of answer fields supply evidence.
/// Keeping the original escapes preserves exact source byte attribution.
pub(crate) fn personal_evidence_spans(message: &Message) -> Vec<&str> {
    personal_evidence_text(&message.role, &message.content)
}

pub(crate) fn personal_evidence_text<'a>(role: &str, raw: &'a str) -> Vec<&'a str> {
    let owner_content = strip_instruction_prefix(raw, KIRO_STEERING_PREFIXES);
    let owner_content = strip_included_rules(owner_content, KIRO_INCLUDED_RULE_GUIDANCE);
    let content = owner_content.trim();
    if !role.eq_ignore_ascii_case("user") || content.is_empty() || declared_transcript(content) {
        return Vec::new();
    }
    let first_line = content.lines().next().unwrap_or("").to_ascii_lowercase();
    if [
        "<environment_context>",
        "<codex_internal_context",
        "<permissions instructions>",
        "<app-context>",
        "# agents.md instructions for ",
    ]
    .iter()
    .any(|prefix| first_line.starts_with(prefix))
    {
        return Vec::new();
    }
    const START: &str = "<send_user_message_question_reply>";
    const END: &str = "</send_user_message_question_reply>";
    if !content.starts_with(START) {
        return vec![owner_content];
    }
    #[derive(Deserialize)]
    struct Reply<'a> {
        #[serde(borrow)]
        answer: &'a serde_json::value::RawValue,
    }
    let Some(payload) = content
        .strip_prefix(START)
        .and_then(|s| s.strip_suffix(END))
    else {
        return Vec::new();
    };
    let Ok(replies) = serde_json::from_str::<Vec<Reply<'_>>>(payload) else {
        return Vec::new();
    };
    replies
        .into_iter()
        .filter_map(|reply| {
            let raw = reply.answer.get();
            let decoded: String = serde_json::from_str(raw).ok()?;
            if decoded.trim().is_empty() || declared_transcript(&decoded) {
                return None;
            }
            raw.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
        })
        .collect()
}

fn declared_transcript(content: &str) -> bool {
    let first_line = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    matches!(
        first_line.as_str(),
        "this was it:"
            | "here is the transcript:"
            | "here's the transcript:"
            | "here is the conversation:"
            | "here's the conversation:"
            | "example conversation:"
    )
}

fn direct_statements(content: &str) -> Vec<&str> {
    direct_sentences(content)
        .into_iter()
        .filter(|part| !part.ends_with('?'))
        .collect()
}

fn direct_sentences(content: &str) -> Vec<&str> {
    if declared_transcript(content) {
        // A pasted transcript is source material, not the owner's endorsement.
        // The rules cannot resolve individual speakers inside it reliably.
        return Vec::new();
    }
    let mut statements = Vec::new();
    let mut code_fence: Option<&str> = None;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            let marker = &line[..3];
            if code_fence == Some(marker) {
                code_fence = None;
            } else if code_fence.is_none() {
                code_fence = Some(marker);
            }
            continue;
        }
        if code_fence.is_some()
            || raw_line.starts_with("    ")
            || raw_line.starts_with('\t')
            || line.starts_with(['>', '"', '\'', '“', '‘', '`'])
            || line.is_empty()
        {
            continue;
        }
        let line = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line);
        // Sentence boundaries stay byte-safe for Unicode. A question never
        // becomes a stated preference merely because it has a familiar prefix.
        for part in sentence_spans(line) {
            let part = part.trim();
            if !part.is_empty() {
                statements.push(part);
            }
        }
    }
    statements
}

/// Describe repeated recorded requests, never an enduring preference or habit.
/// Different source messages and different question text are both necessary.
pub(super) fn comparison_requests(
    conversation: &Conversation,
    revision: i64,
) -> Option<CandidateObservation> {
    if check_text(&conversation.provider, "provider", 512).is_err() {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut evidence = Vec::new();
    for message in &conversation.messages {
        if check_text(&message.source_id, "message ID", 4096).is_err() {
            continue;
        }
        for quote in personal_evidence_spans(message)
            .into_iter()
            .flat_map(direct_sentences)
        {
            if quote.chars().count() > 500
                || quote.contains(crate::privacy::REDACTED)
                || quote.contains('\0')
            {
                continue;
            }
            let normalized = normalize_key(quote).replace('’', "'");
            let question_key = normalized
                .split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if is_comparison_request(&normalized) && seen.insert(question_key) {
                evidence.push(evidence_for(conversation, revision, message, quote));
                break;
            }
        }
        if evidence.len() == 1000 {
            break;
        }
    }
    if evidence
        .iter()
        .map(|e| &e.source_message_id)
        .collect::<BTreeSet<_>>()
        .len()
        < 2
    {
        return None;
    }
    let mut observation = CandidateObservation {
        kind: ExtractionKind::BehavioralPattern,
        inference: InferenceType::Inferred,
        subject: "user".into(),
        topic: "interaction_pattern".into(),
        canonical_value: "comparison requests in recorded interactions".into(),
        context: None,
        summary: String::new(),
        decision_event: None,
        evidence,
    };
    observation.summary = render_identity_summary(&observation);
    Some(observation)
}

pub(super) fn is_comparison_request(text: &str) -> bool {
    let text = text.strip_prefix("please ").unwrap_or(text);
    if ["don't ", "do not ", "not ", "never ", "stop comparing"]
        .iter()
        .any(|negative| text.contains(negative))
    {
        return false;
    }
    let choice = text.starts_with("which ")
        && text.ends_with('?')
        && text
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| matches!(word, "best" | "better" | "more"));
    let explicit = [
        "compare ",
        "can you compare ",
        "could you compare ",
        "would you compare ",
    ]
    .iter()
    .any(|prefix| text.starts_with(prefix))
        && [" and ", " versus ", " vs ", " with ", " to "]
            .iter()
            .any(|separator| text.contains(separator));
    if choice || explicit {
        return true;
    }
    // A bounded noun-phrase choice with a stated use case, e.g. A or B for X?
    let Some((left, right)) = text.strip_suffix('?').and_then(|t| t.split_once(" or ")) else {
        return false;
    };
    if !right.contains(" for ") || right.contains(" or ") || left.is_empty() || right.is_empty() {
        return false;
    }
    let forbidden = [
        "i", "we", "you", "they", "he", "she", "it", "if", "suppose", "example", "how", "why",
        "when", "where", "which", "is", "are", "was", "were", "has", "have", "can", "could", "do",
        "does", "should", "would", "not", "don't", "never",
    ];
    !left
        .split_whitespace()
        .any(|word| forbidden.contains(&word))
}

pub(crate) fn sentence_spans(line: &str) -> Vec<&str> {
    let mut start = 0;
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if matches!(ch, '.' | '!' | '?')
            && chars.peek().is_none_or(|(_, next)| next.is_whitespace())
        {
            let end = index + ch.len_utf8();
            spans.push(&line[start..end]);
            start = end;
        }
    }
    if start < line.len() {
        spans.push(&line[start..]);
    }
    spans
}

fn classify_statement(quote: &str) -> Option<CandidateObservation> {
    let normalized = normalize_sentence(quote);
    let statement = normalized.strip_prefix("again, ").unwrap_or(&normalized);
    let mut observation = CandidateObservation {
        kind: ExtractionKind::Correction,
        inference: InferenceType::Explicit,
        subject: "user".to_owned(),
        topic: "interaction_instruction".to_owned(),
        canonical_value: statement.to_owned(),
        context: None,
        summary: format!("Requested: {quote}"),
        decision_event: None,
        evidence: Vec::new(),
    };
    for (prefix, polarity) in [
        ("i prefer ", "prefer"),
        ("i don't like ", "dislike"),
        ("i do not like ", "dislike"),
    ] {
        if let Some(value) = statement.strip_prefix(prefix) {
            if !value.chars().any(char::is_alphanumeric) {
                return None;
            }
            observation.kind = ExtractionKind::ExplicitPreference;
            observation.topic = "stated_preference".to_owned();
            observation.canonical_value = format!("{polarity} {value}");
            observation.summary = format!("Stated preference: {quote}");
            return Some(observation);
        }
    }
    for (prefix, event) in [
        ("i am reconsidering ", DecisionEventType::Reconsideration),
        ("i'm reconsidering ", DecisionEventType::Reconsideration),
        ("i reconsidered ", DecisionEventType::Reconsideration),
        ("i am considering ", DecisionEventType::Consideration),
        ("i'm considering ", DecisionEventType::Consideration),
        ("i considered ", DecisionEventType::Consideration),
        ("i propose ", DecisionEventType::Proposal),
        ("i suggest ", DecisionEventType::Proposal),
        ("i reaffirm ", DecisionEventType::Confirmation),
        ("i confirm ", DecisionEventType::Confirmation),
        ("i reversed my decision ", DecisionEventType::Reversal),
        ("i changed my mind ", DecisionEventType::Reversal),
        ("i chose ", DecisionEventType::Decision),
        ("i have decided ", DecisionEventType::Decision),
        ("i've decided ", DecisionEventType::Decision),
        ("i decided ", DecisionEventType::Decision),
    ] {
        if let Some(value) = statement.strip_prefix(prefix) {
            if !value.chars().any(char::is_alphanumeric) {
                return None;
            }
            observation.kind = ExtractionKind::Decision;
            observation.topic = "stated_decision".to_owned();
            observation.canonical_value = value.to_owned();
            observation.summary = format!("Decision history ({event:?}): {quote}");
            observation.decision_event = Some(event);
            return Some(observation);
        }
    }
    let imperative = [
        "keep ",
        "don't ",
        "do not ",
        "stop ",
        "avoid ",
        "reuse ",
        "give me shorter ",
    ];
    if !imperative
        .iter()
        .any(|prefix| statement.starts_with(prefix))
    {
        return None;
    }
    if let Some((value, topic)) = canonical_correction(statement) {
        observation.canonical_value = value.to_owned();
        observation.topic = topic.to_owned();
    }
    Some(observation)
}

fn canonical_correction(statement: &str) -> Option<(&'static str, &'static str)> {
    // Exact whole-statement equivalences only. An extra project, condition or
    // exception makes the statement distinct rather than silently generalizing.
    match statement {
        "keep implementations simple"
        | "keep implementation simple"
        | "keep the implementation simple"
        | "keep code simple"
        | "keep the code simple" => Some(("keep_implementations_simple", "software_design")),
        "don't introduce additional dependencies"
        | "do not introduce additional dependencies"
        | "don't add dependencies"
        | "do not add dependencies"
        | "don't add new dependencies"
        | "do not add new dependencies" => Some(("avoid_new_dependencies", "software_design")),
        "stop creating unnecessary markdown files"
        | "don't create unnecessary markdown files"
        | "do not create unnecessary markdown files" => {
            Some(("avoid_unnecessary_markdown_files", "software_design"))
        }
        "reuse existing components" | "reuse the existing components" => {
            Some(("reuse_existing_components", "software_design"))
        }
        "don't rewrite working code" | "do not rewrite working code" => {
            Some(("preserve_working_code", "software_design"))
        }
        "give me shorter explanations" | "keep explanations concise" => {
            Some(("concise_explanations", "communication.explanations"))
        }
        _ => None,
    }
}

/// Check the sentence containing a quotation, instead of excluding a whole
/// mixed message because its opening sentence asks about an option. Questions
/// can be rhetorical corrections; ordinary questions cannot state preferences.
pub(crate) fn statement_quote_supported(
    message: &Message,
    quote: &str,
    kind: ExtractionKind,
) -> bool {
    personal_evidence_spans(message)
        .into_iter()
        .flat_map(str::lines)
        .flat_map(sentence_spans)
        .any(|sentence| {
            let sentence = sentence.trim();
            !sentence.is_empty()
                && !is_option_question(sentence)
                && !(kind == ExtractionKind::ExplicitPreference && sentence.ends_with('?'))
                && (sentence.contains(quote) || quote.contains(sentence))
        })
}

pub(crate) fn is_option_question(quote: &str) -> bool {
    let quote = normalize_key(quote);
    [
        "what about ",
        "how about ",
        "could we use ",
        "should we use ",
        "would it make sense to use ",
    ]
    .iter()
    .any(|prefix| quote.starts_with(prefix))
}

pub(super) fn normalize_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalize_sentence(value: &str) -> String {
    normalize_key(value.trim_end_matches(['.', '!'])).replace('’', "'")
}
