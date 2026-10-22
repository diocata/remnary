//! Provider-independent, evidence-grounded extraction contracts.
//!
//! Version 1 uses strict JSON objects. All normalized observation fields are
//! required; generation produces canonical claim fields and Rust renders the
//! description. `context` and `decision_event` may be null. Every observation
//! remains a candidate until reviewed. A valid citation proves provenance, not
//! semantic truth: callers must continue to label model inferences as inferences.
//! The built-in extractor recognizes a deliberately small set of direct user
//! statements. It does not run a language model or infer psychological traits.

mod rules;

pub use rules::extract_rules;
#[cfg(test)]
use rules::{
    comparison_requests, is_comparison_request, strip_included_rules, strip_instruction_prefix,
};
pub(crate) use rules::{
    eligible_personal_evidence, personal_evidence_spans, personal_evidence_text, sentence_spans,
    statement_quote_supported,
};
use rules::{evidence_for, normalize_key};

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use crate::model::{Conversation, digest};

pub const SCHEMA_VERSION: u32 = 1;
// Attribution and detector changes must invalidate cached rule extractions.
pub const RULES_VERSION: &str = "explicit-rules-v7";
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
    /// Remnary's stable conversation ID, not the provider's external source ID.
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

/// Propose literal feedback omitted by generation. These are candidates only:
/// the model pipeline must independently verify classification and attribution.
/// Keeping the full sentence avoids inventing an enduring preference or scope.
pub(crate) fn propose_literal_feedback(
    conversation: &Conversation,
    revision: i64,
    mut output: AnalysisOutput,
) -> AnalysisOutput {
    static ACTION: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let action = ACTION.get_or_init(|| {
        regex::Regex::new(r"(?i)\byou\s+(?:[\p{L}]+ed|did|made|wrote|ran|sent|took|broke|went)\b")
            .expect("fixed feedback action pattern")
    });
    for message in &conversation.messages {
        if output.observations.len() >= 8 {
            break;
        }
        if output.observations.iter().any(|o| {
            o.kind == ExtractionKind::Correction
                && o.evidence
                    .iter()
                    .any(|e| e.source_message_id == message.source_id)
        }) {
            continue;
        }
        let sentence = personal_evidence_spans(message)
            .into_iter()
            .flat_map(str::lines)
            .flat_map(sentence_spans)
            .map(str::trim)
            .find(|quote| {
                let text = quote.to_lowercase().replace('’', "'");
                quote.len() <= 3000
                    && !quote.ends_with('?')
                    && !["if ", "what if ", "suppose "]
                        .iter()
                        .any(|p| text.starts_with(p))
                    && action.is_match(quote)
                    && [
                        "without asking",
                        "without my approval",
                        "without permission",
                        "after i asked",
                        "after i told",
                        "i asked you not to",
                        "i told you not to",
                    ]
                    .iter()
                    .any(|cue| text.contains(cue))
            });
        if let Some(quote) = sentence {
            let mut observation = CandidateObservation {
                kind: ExtractionKind::Correction,
                inference: InferenceType::Explicit,
                subject: "user".into(),
                topic: "assistant_actions".into(),
                canonical_value: quote.into(),
                context: None,
                summary: String::new(),
                decision_event: None,
                evidence: vec![evidence_for(conversation, revision, message, quote)],
            };
            observation.summary = render_identity_summary(&observation);
            output.observations.push(observation);
        }
    }
    output
}

/// Fresh model scopes must occur in their own citations, never just another
/// message in the window. Legacy reviewed identities may use paraphrased scopes
/// and are independently checked against new evidence during reconciliation.
pub(crate) fn retain_cited_model_scopes(output: &mut AnalysisOutput) -> usize {
    let before = output.observations.len();
    output.observations.retain(|observation| {
        observation.context.as_ref().is_none_or(|context| {
            let context = normalize_key(context);
            observation
                .evidence
                .iter()
                .any(|evidence| normalize_key(&evidence.quote).contains(&context))
        })
    });
    before - output.observations.len()
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
mod tests;
