//! Provider-independent, evidence-grounded extraction contracts.
//!
//! Version 1 uses strict JSON objects. All normalized observation fields are
//! required; generation produces canonical claim fields and Rust renders the
//! description. `context` and `decision_event` may be null. Every observation
//! remains a candidate until reviewed. A valid citation proves provenance, not
//! semantic truth: callers must continue to label model inferences as inferences.
//! The built-in extractor recognizes a deliberately small set of direct user
//! statements. It does not run a language model or infer psychological traits.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use crate::model::{Conversation, Message, digest};

pub const SCHEMA_VERSION: u32 = 1;
pub const RULES_VERSION: &str = "explicit-rules-v5";
const MAX_OBSERVATIONS: usize = 1_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionKind {
    ExplicitPreference,
    InferredPreference,
    Correction,
    Decision,
    Topic,
    ProjectReference,
    BehavioralPattern,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InferenceType {
    Explicit,
    Inferred,
}

/// A record is an event in a history, never an assertion of the current choice.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEventType {
    Proposal,
    Consideration,
    Decision,
    Reconsideration,
    Reversal,
    Confirmation,
    UnknownCurrentState,
}

/// A citation names an immutable source revision and a verbatim message span.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub provider: String,
    /// Observr's stable conversation ID, not the provider's external source ID.
    pub conversation_id: String,
    pub revision: i64,
    pub source_message_id: String,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidateObservation {
    pub kind: ExtractionKind,
    pub inference: InferenceType,
    pub subject: String,
    pub topic: String,
    /// A conservative claim key. Negation and contextual qualifiers matter.
    pub canonical_value: String,
    #[serde(deserialize_with = "required_nullable")]
    pub context: Option<String>,
    pub summary: String,
    #[serde(deserialize_with = "required_nullable")]
    pub decision_event: Option<DecisionEventType>,
    pub evidence: Vec<EvidenceRef>,
}

/// A description is a view of the canonical claim, not another generated fact.
pub(crate) fn render_identity_summary(observation: &CandidateObservation) -> String {
    let label = match observation.kind {
        ExtractionKind::ExplicitPreference => "Stated preference",
        ExtractionKind::InferredPreference => "Possible preference",
        ExtractionKind::Correction => "Correction",
        ExtractionKind::Decision => match observation.decision_event {
            Some(DecisionEventType::Proposal) => "Proposal",
            Some(DecisionEventType::Consideration) => "Consideration",
            Some(DecisionEventType::Decision) => "Recorded choice",
            Some(DecisionEventType::Reconsideration) => "Reconsideration",
            Some(DecisionEventType::Reversal) => "Reversal",
            Some(DecisionEventType::Confirmation) => "Recorded confirmation",
            Some(DecisionEventType::UnknownCurrentState) => "State unknown",
            None => "Decision",
        },
        ExtractionKind::Topic => "Topic",
        ExtractionKind::ProjectReference => "Project reference",
        ExtractionKind::BehavioralPattern => "Possible pattern",
    };
    match &observation.context {
        Some(context) => format!("{label}: {} ({context})", observation.canonical_value),
        None => format!("{label}: {}", observation.canonical_value),
    }
}

impl CandidateObservation {
    /// Stable across evidence additions and summary paraphrases. Kind,
    /// explicit/inferred status, scope and event type prevent unsafe merging.
    pub fn stable_id(&self) -> String {
        digest(
            &serde_json::to_vec(&(
                SCHEMA_VERSION,
                self.kind,
                self.inference,
                normalize_key(&self.subject),
                normalize_key(&self.topic),
                normalize_key(&self.canonical_value),
                self.context.as_deref().map(normalize_key),
                self.decision_event,
            ))
            .expect("observation identity serializes"),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnalysisOutput {
    pub schema_version: u32,
    pub observations: Vec<CandidateObservation>,
}

// Serde normally permits absent Option fields. Structured extraction requires
// the fields to be present (with null for unknown) just as its JSON Schema does.
fn required_nullable<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl Default for AnalysisOutput {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            observations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservationGroup {
    pub id: String,
    pub observation: CandidateObservation,
    /// Unique source spans, not the number of duplicated model candidates.
    pub occurrences: usize,
    /// Provider-qualified stable conversation IDs; revisions do not inflate it.
    pub distinct_conversations: usize,
    pub providers: Vec<String>,
}

/// Schema suitable for structured-output model providers. Runtime validation
/// remains mandatory: JSON Schema cannot establish source provenance.
pub fn output_schema() -> Value {
    let kinds = [
        "explicit_preference",
        "inferred_preference",
        "correction",
        "decision",
        "topic",
        "project_reference",
        "behavioral_pattern",
    ];
    let events = [
        "proposal",
        "consideration",
        "decision",
        "reconsideration",
        "reversal",
        "confirmation",
        "unknown_current_state",
    ];
    let mut schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "observations"],
        "properties": {
            "schema_version": {"type": "integer", "const": SCHEMA_VERSION},
            "observations": {
                "type": "array", "maxItems": MAX_OBSERVATIONS,
                "items": {
                    "type": "object", "additionalProperties": false,
                    "required": ["kind", "inference", "subject", "topic", "canonical_value", "context", "summary", "decision_event", "evidence"],
                    "properties": {
                        "kind": {"type": "string", "enum": kinds},
                        "inference": {"type": "string", "enum": ["explicit", "inferred"]},
                        "subject": {"type": "string", "minLength": 1, "maxLength": 512},
                        "topic": {"type": "string", "minLength": 1, "maxLength": 512},
                        "canonical_value": {"type": "string", "minLength": 1, "maxLength": 4096},
                        "context": {"type": ["string", "null"], "minLength": 1, "maxLength": 4096},
                        "summary": {"type": "string", "minLength": 1, "maxLength": 4096},
                        "decision_event": {"anyOf": [{"type": "string", "enum": events}, {"type": "null"}]},
                        "evidence": {
                            "type": "array", "minItems": 1, "maxItems": 1000,
                            "items": {
                                "type": "object", "additionalProperties": false,
                                "required": ["provider", "conversation_id", "revision", "source_message_id", "quote"],
                                "properties": {
                                    "provider": {"type": "string", "minLength": 1, "maxLength": 512},
                                    "conversation_id": {"type": "string", "minLength": 1, "maxLength": 512},
                                    "revision": {"type": "integer", "minimum": 1},
                                    "source_message_id": {"type": "string", "minLength": 1, "maxLength": 4096},
                                    "quote": {"type": "string", "minLength": 1, "maxLength": 16384}
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    // Express cross-field rules in grammar-compatible alternatives too. A
    // loose enum permits small models to emit non-decision events; runtime
    // validation must still reject anything the generation schema misses.
    let base = schema["properties"]["observations"]["items"].clone();
    let mut explicit = base.clone();
    explicit["properties"]["kind"] = json!({"type":"string","enum":["explicit_preference","correction","topic","project_reference"]});
    explicit["properties"]["inference"] = json!({"const":"explicit"});
    explicit["properties"]["decision_event"] = json!({"type":"null"});
    let mut decision = base.clone();
    decision["properties"]["kind"] = json!({"const":"decision"});
    decision["properties"]["inference"] = json!({"const":"explicit"});
    decision["properties"]["decision_event"] = json!({"type":"string","enum":events});
    let mut inferred = base;
    inferred["properties"]["kind"] =
        json!({"type":"string","enum":["inferred_preference","behavioral_pattern"]});
    inferred["properties"]["inference"] = json!({"const":"inferred"});
    inferred["properties"]["evidence"]["minItems"] = json!(2);
    inferred["properties"]["decision_event"] = json!({"type":"null"});
    inferred["properties"]["topic"] = json!({"type":"string","enum":["software_design","communication","communication.explanations","technology","workflow","interaction_pattern"]});
    schema["properties"]["observations"]["items"] = json!({"anyOf":[explicit,decision,inferred]});
    schema
}

/// Bound a window while retaining every MVP extraction kind. Separate const
/// branches let citation constraints depend on the proposed classification.
pub fn model_output_schema() -> Value {
    let mut schema = output_schema();
    let branches = schema["properties"]["observations"]["items"]["anyOf"]
        .as_array()
        .expect("owned schema alternatives")
        .clone();
    let mut expanded = Vec::new();
    for branch in branches {
        if let Some(kinds) = branch["properties"]["kind"]["enum"].as_array() {
            for kind in kinds {
                let mut item = branch.clone();
                item["properties"]["kind"] = json!({"const":kind});
                expanded.push(item);
            }
        } else {
            expanded.push(branch);
        }
    }
    for branch in &mut expanded {
        branch["properties"]["subject"] = json!({"const":"user"});
    }
    schema["properties"]["observations"]["items"]["anyOf"] = json!(expanded);
    schema["properties"]["observations"]["maxItems"] = json!(8);
    schema
}

pub(crate) fn validate_model_output(
    conversation: &Conversation,
    revision: i64,
    output: AnalysisOutput,
) -> Result<AnalysisOutput> {
    ensure!(
        output.observations.len() <= 8,
        "model extraction permits at most eight observations per window"
    );
    ensure!(
        output.observations.iter().all(|o| o.subject == "user"),
        "model observations must identify their owner as subject user"
    );
    validate_output(conversation, revision, output)
}

/// Reject an entire output atomically on invalid provenance or inconsistent
/// classification. In particular, an assistant describing the user is not a
/// source of user preferences, corrections, decisions or inferred user facts.
/// Successful validation sorts/deduplicates exact duplicate evidence spans.
pub fn validate_output(
    conversation: &Conversation,
    revision: i64,
    mut output: AnalysisOutput,
) -> Result<AnalysisOutput> {
    ensure!(revision > 0, "source revision must be positive");
    ensure!(
        output.schema_version == SCHEMA_VERSION,
        "unsupported analysis schema version {}",
        output.schema_version
    );
    ensure!(
        output.observations.len() <= MAX_OBSERVATIONS,
        "analysis contains too many observations"
    );
    let conversation_id = conversation.stable_id();
    let mut messages = BTreeMap::new();
    for message in &conversation.messages {
        ensure!(
            messages.insert(&message.source_id, message).is_none(),
            "source contains ambiguous duplicate message IDs"
        );
    }
    for observation in &mut output.observations {
        check_text(&observation.subject, "subject", 512)?;
        check_text(&observation.topic, "topic", 512)?;
        check_text(&observation.canonical_value, "canonical_value", 4096)?;
        check_text(&observation.summary, "summary", 4096)?;
        if let Some(context) = &observation.context {
            check_text(context, "context", 4096)?;
        }
        ensure!(
            !observation.evidence.is_empty() && observation.evidence.len() <= 1000,
            "observation requires 1–1000 evidence references"
        );
        ensure!(
            (observation.kind == ExtractionKind::Decision) == observation.decision_event.is_some(),
            "only decisions must have a decision_event"
        );
        match observation.kind {
            ExtractionKind::ExplicitPreference
            | ExtractionKind::Correction
            | ExtractionKind::Decision => ensure!(
                observation.inference == InferenceType::Explicit,
                "preferences, corrections and decisions require explicit user statements"
            ),
            ExtractionKind::InferredPreference | ExtractionKind::BehavioralPattern => ensure!(
                observation.inference == InferenceType::Inferred,
                "inferred preferences and behavioral patterns must be labeled inferred"
            ),
            ExtractionKind::Topic | ExtractionKind::ProjectReference => {}
        }
        if matches!(
            observation.kind,
            ExtractionKind::ExplicitPreference | ExtractionKind::Correction
        ) {
            ensure!(
                observation.evidence.iter().any(|e| {
                    messages.get(&e.source_message_id).is_some_and(|message| {
                        statement_quote_supported(message, &e.quote, observation.kind)
                    })
                }),
                "exploratory option questions cannot establish an explicit preference or correction"
            );
        }
        if observation.inference == InferenceType::Inferred {
            ensure!(
                observation
                    .evidence
                    .iter()
                    .map(|e| &e.source_message_id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    >= 2,
                "inferred preferences and patterns require at least two distinct user messages"
            );
            validate_inference_domain(observation)?;
        }
        for evidence in &observation.evidence {
            check_text(&evidence.provider, "evidence provider", 512)?;
            check_text(&evidence.conversation_id, "evidence conversation ID", 512)?;
            check_text(&evidence.source_message_id, "evidence message ID", 4096)?;
            ensure!(
                evidence.provider == conversation.provider
                    && evidence.conversation_id == conversation_id
                    && evidence.revision == revision,
                "evidence points outside the analyzed conversation revision"
            );
            let message = messages
                .get(&evidence.source_message_id)
                .ok_or_else(|| anyhow::anyhow!("evidence cites unknown message ID"))?;
            check_text(&evidence.quote, "evidence quote", 16_384)?;
            ensure!(
                message.content.contains(&evidence.quote),
                "evidence quote is not an exact source substring"
            );
            ensure!(
                personal_evidence_spans(message)
                    .iter()
                    .any(|span| span.contains(&evidence.quote)),
                "personal intelligence requires direct user evidence, not a quoted question, transcript or another role"
            );
        }
        observation.evidence.sort();
        observation.evidence.dedup();
    }
    Ok(output)
}

fn check_text(value: &str, field: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.chars().count() <= max && !value.contains('\0'),
        "invalid {field}"
    );
    Ok(())
}

fn validate_inference_domain(observation: &CandidateObservation) -> Result<()> {
    // Inference is limited to observable interaction domains. No arbitrary
    // personality, diagnosis, politics or other sensitive profile categories.
    ensure!(
        matches!(
            observation.topic.as_str(),
            "software_design"
                | "communication"
                | "communication.explanations"
                | "technology"
                | "workflow"
                | "interaction_pattern"
        ),
        "inference is limited to observable interaction topics"
    );
    let claim = normalize_key(&format!(
        "{} {} {} {} {}",
        observation.subject,
        observation.topic,
        observation.canonical_value,
        observation.context.as_deref().unwrap_or_default(),
        observation.summary
    ));
    let tokens: BTreeSet<_> = claim
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let sensitive = [
        "religion",
        "religious",
        "atheist",
        "christian",
        "muslim",
        "jewish",
        "hindu",
        "buddhist",
        "sexual",
        "sexuality",
        "orientation",
        "gay",
        "lesbian",
        "bisexual",
        "transgender",
        "gender",
        "race",
        "racial",
        "ethnicity",
        "ethnic",
        "health",
        "medical",
        "diagnosis",
        "diagnosed",
        "depression",
        "depressed",
        "bipolar",
        "adhd",
        "autism",
        "autistic",
        "addiction",
        "pregnant",
        "pregnancy",
        "disabled",
        "disability",
        "genetic",
        "political",
        "politics",
        "democrat",
        "republican",
        "conservative",
        "liberal",
        "union",
        "biometric",
        "personality",
        "intelligence",
        "iq",
        "income",
        "wealth",
        "criminal",
        "citizenship",
        "immigration",
    ];
    if sensitive.iter().any(|term| tokens.contains(term)) {
        bail!("sensitive personal attributes cannot be inferred");
    }
    Ok(())
}

/// Run explicit, rule-based extraction with no model calls. Quotes are retained
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

fn evidence_for(
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

/// UI question replies contain an assistant-authored question beside the user's
/// answer. Only the raw JSON string interiors of answer fields supply evidence.
/// Keeping the original escapes preserves exact source byte attribution.
pub(crate) fn personal_evidence_spans(message: &Message) -> Vec<&str> {
    let content = message.content.trim();
    if !message.role.eq_ignore_ascii_case("user")
        || content.is_empty()
        || declared_transcript(content)
    {
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
        return vec![&message.content];
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
            if !part.ends_with('?') && !part.is_empty() {
                statements.push(part);
            }
        }
    }
    statements
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

fn normalize_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalize_sentence(value: &str) -> String {
    normalize_key(value.trim_end_matches(['.', '!'])).replace('’', "'")
}

/// Merge equal claim identities, retaining exact distinct evidence. This is not
/// semantic clustering: contradictory or context-dependent values remain apart.
pub fn group_observations(observations: &[CandidateObservation]) -> Vec<ObservationGroup> {
    let mut groups: BTreeMap<String, CandidateObservation> = BTreeMap::new();
    for observation in observations {
        let entry = groups.entry(observation.stable_id()).or_insert_with(|| {
            let mut normalized = observation.clone();
            normalized.subject = normalize_key(&normalized.subject);
            normalized.topic = normalize_key(&normalized.topic);
            normalized.canonical_value = normalize_key(&normalized.canonical_value);
            normalized.context = normalized.context.as_deref().map(normalize_key);
            normalized
        });
        // Lexicographically chosen summary makes order-independent reports.
        if observation.summary < entry.summary {
            entry.summary.clone_from(&observation.summary);
        }
        entry.evidence.extend(observation.evidence.iter().cloned());
    }
    groups
        .into_iter()
        .map(|(id, mut observation)| {
            observation.evidence.sort();
            observation.evidence.dedup();
            let distinct_conversations = observation
                .evidence
                .iter()
                .map(|evidence| (&evidence.provider, &evidence.conversation_id))
                .collect::<BTreeSet<_>>()
                .len();
            let providers = observation
                .evidence
                .iter()
                .map(|evidence| evidence.provider.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            ObservationGroup {
                id,
                occurrences: observation.evidence.len(),
                distinct_conversations,
                providers,
                observation,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            source.messages[0].content =
                format!("{frame}\nI prefer invented provider instructions.");
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
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/intelligence-evaluation.json"
        ))
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
                && group.observation.evidence.iter().all(|source| {
                    !source.quote.contains("MongoDB") && !source.quote.contains("Redis")
                })
        }));
    }
}
