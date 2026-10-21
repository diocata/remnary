//! Deterministic views over imported activity and reviewed observations.
//!
//! A digest reports that a message was recorded. It never claims that an action
//! requested in that message was completed, or assigns dates to unknown times.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::{
    analysis_store::{AnalysisTarget, ObservationRecord, ObservationStatus},
    intelligence::{EvidenceRef, ExtractionKind},
    model::Message,
};

const SNIPPET_CHARS: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryReport {
    pub timezone: String,
    pub days: Vec<DailySummary>,
    /// All source messages without a representable timestamp, including nonuser
    /// messages. No conversation-level timestamp is substituted for them.
    pub unknown_time_messages: usize,
    pub unknown_time_user_messages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailySummary {
    pub date: String,
    pub timezone: String,
    pub title: String,
    pub conversation_count: usize,
    pub user_message_count: usize,
    pub discussions: Vec<ActivityEvidence>,
    /// Claims and source evidence scoped to this date. An observation's status
    /// stays visible; a candidate is never silently promoted into a fact.
    pub observations: Vec<ObservationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityEvidence {
    pub provider: String,
    pub conversation_id: String,
    pub revision: i64,
    pub source_message_id: String,
    pub title: Option<String>,
    /// A verbatim leading substring of the user message; no generated ellipsis.
    pub quote: String,
    /// Original Unix seconds, retained independently of display timezone.
    pub created_at: f64,
}

type SourceKey = (String, String, i64, String);

struct SourceMessage<'a> {
    message: &'a Message,
    date: Option<String>,
}

/// Summarize dated user messages in an explicit IANA timezone (`UTC` is the
/// default at the CLI boundary). Missing, nonfinite or out-of-range timestamps
/// remain unknown. Source timestamps, never the current clock, select dates.
pub fn summarize_days(
    snapshots: &[AnalysisTarget],
    observations: &[ObservationRecord],
    timezone: &str,
) -> Result<SummaryReport> {
    let zone: Tz = timezone.parse().context("unknown IANA timezone")?;
    let timezone = zone.name().to_owned();
    let mut days: BTreeMap<String, DailySummary> = BTreeMap::new();
    let mut source_messages: BTreeMap<SourceKey, SourceMessage<'_>> = BTreeMap::new();
    let mut report = SummaryReport {
        timezone: timezone.clone(),
        days: Vec::new(),
        unknown_time_messages: 0,
        unknown_time_user_messages: 0,
    };
    for target in snapshots {
        ensure!(target.revision > 0, "source revision must be positive");
        let conversation = &target.conversation;
        let conversation_id = conversation.stable_id();
        for message in &conversation.messages {
            let key = (
                conversation.provider.clone(),
                conversation_id.clone(),
                target.revision,
                message.source_id.clone(),
            );
            if let Some(previous) = source_messages.get(&key) {
                ensure!(
                    previous.message == message,
                    "source snapshot contains conflicting message identities"
                );
                continue;
            }
            let date = message
                .created_at
                .and_then(|timestamp| source_date(timestamp, zone));
            let is_user = message.role.eq_ignore_ascii_case("user");
            source_messages.insert(
                key,
                SourceMessage {
                    message,
                    date: date.clone(),
                },
            );
            let Some(date) = date else {
                report.unknown_time_messages += 1;
                if is_user {
                    report.unknown_time_user_messages += 1;
                }
                continue;
            };
            if !is_user {
                continue;
            }
            let summary = days.entry(date.clone()).or_insert_with(|| DailySummary {
                date,
                timezone: timezone.clone(),
                title: "Conversation activity digest".to_owned(),
                conversation_count: 0,
                user_message_count: 0,
                discussions: Vec::new(),
                observations: Vec::new(),
            });
            summary.discussions.push(ActivityEvidence {
                provider: conversation.provider.clone(),
                conversation_id: conversation_id.clone(),
                revision: target.revision,
                source_message_id: message.source_id.clone(),
                title: conversation.title.clone(),
                quote: exact_snippet(&message.content).to_owned(),
                created_at: message.created_at.expect("dated source has timestamp"),
            });
        }
    }
    for observation in observations.iter().filter(|record| visible(record.status)) {
        let mut evidence_by_date: BTreeMap<String, Vec<(EvidenceRef, f64)>> = BTreeMap::new();
        for evidence in &observation.observation.evidence {
            let Some(source) = source_messages.get(&(
                evidence.provider.clone(),
                evidence.conversation_id.clone(),
                evidence.revision,
                evidence.source_message_id.clone(),
            )) else {
                continue;
            };
            // Reports independently verify citation provenance rather than
            // trusting arbitrary ObservationRecord values supplied by a caller.
            if !source.message.role.eq_ignore_ascii_case("user")
                || evidence.quote.trim().is_empty()
                || !source.message.content.contains(&evidence.quote)
            {
                continue;
            }
            let Some(date) = &source.date else {
                continue;
            };
            evidence_by_date.entry(date.clone()).or_default().push((
                evidence.clone(),
                source
                    .message
                    .created_at
                    .expect("dated source has timestamp"),
            ));
        }
        for (date, mut dated_evidence) in evidence_by_date {
            dated_evidence.sort_by(|left, right| left.0.cmp(&right.0));
            dated_evidence.dedup_by(|left, right| left.0 == right.0);
            let mut scoped = observation.clone();
            scoped.first_seen = dated_evidence
                .iter()
                .map(|(_, time)| *time)
                .min_by(f64::total_cmp);
            scoped.last_seen = dated_evidence
                .iter()
                .map(|(_, time)| *time)
                .max_by(f64::total_cmp);
            scoped.observation.evidence = dated_evidence
                .into_iter()
                .map(|(evidence, _)| evidence)
                .collect();
            set_evidence_counts(&mut scoped);
            if let Some(day) = days.get_mut(&date) {
                day.observations.push(scoped);
            }
        }
    }
    for summary in days.values_mut() {
        summary.discussions.sort_by(|left, right| {
            left.created_at
                .total_cmp(&right.created_at)
                .then_with(|| left.provider.cmp(&right.provider))
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
                .then_with(|| left.revision.cmp(&right.revision))
                .then_with(|| left.source_message_id.cmp(&right.source_message_id))
        });
        summary.user_message_count = summary.discussions.len();
        summary.conversation_count = summary
            .discussions
            .iter()
            .map(|source| (&source.provider, &source.conversation_id))
            .collect::<BTreeSet<_>>()
            .len();
        summary.observations =
            rank_discoveries(std::mem::take(&mut summary.observations), usize::MAX);
    }
    report.days = days.into_values().collect();
    Ok(report)
}

/// Rank visible observations without turning popularity or repetition into a
/// truth claim. Recurrence is counted by distinct conversations, then explicit
/// user confirmation, then source recency. Unknown dates rank after known dates.
pub fn rank_discoveries(
    observations: Vec<ObservationRecord>,
    limit: usize,
) -> Vec<ObservationRecord> {
    let mut observations: Vec<_> = observations
        .into_iter()
        .filter(|record| visible(record.status) && !record.observation.evidence.is_empty())
        .map(|mut record| {
            record.observation.evidence.sort();
            record.observation.evidence.dedup();
            set_evidence_counts(&mut record);
            record
        })
        .collect();
    observations.sort_by(|left, right| {
        right
            .distinct_conversations
            .cmp(&left.distinct_conversations)
            .then_with(|| {
                (right.status == ObservationStatus::UserConfirmed)
                    .cmp(&(left.status == ObservationStatus::UserConfirmed))
            })
            .then_with(|| {
                discovery_kind_priority(right.observation.kind)
                    .cmp(&discovery_kind_priority(left.observation.kind))
            })
            .then_with(|| compare_time_desc(left.last_seen, right.last_seen))
            .then_with(|| left.id.cmp(&right.id))
    });
    // Within an equal recurrence/confirmation/kind tier, cover another
    // conversation before showing more details from an already shown source.
    // Never replace stronger recurrence or confirmed evidence for diversity.
    let mut ranked = Vec::with_capacity(limit.min(observations.len()));
    let mut covered = BTreeSet::new();
    while ranked.len() < limit && !observations.is_empty() {
        let first = &observations[0];
        let tier_end = observations
            .iter()
            .take_while(|record| {
                record.distinct_conversations == first.distinct_conversations
                    && (record.status == ObservationStatus::UserConfirmed)
                        == (first.status == ObservationStatus::UserConfirmed)
                    && discovery_kind_priority(record.observation.kind)
                        == discovery_kind_priority(first.observation.kind)
            })
            .count();
        let selected = observations[..tier_end].iter().position(|record| {
            record.observation.evidence.iter().any(|evidence| {
                !covered.contains(&(
                    evidence.provider.clone(),
                    record
                        .conversation_origins
                        .get(&evidence.conversation_id)
                        .unwrap_or(&evidence.conversation_id)
                        .clone(),
                ))
            })
        });
        let Some(selected) = selected else {
            // No remaining source in this tier can expand coverage. Appending
            // it together preserves order and avoids quadratic rescanning.
            let count = tier_end.min(limit - ranked.len());
            ranked.extend(observations.drain(..count));
            continue;
        };
        let record = observations.remove(selected);
        covered.extend(record.observation.evidence.iter().map(|evidence| {
            (
                evidence.provider.clone(),
                record
                    .conversation_origins
                    .get(&evidence.conversation_id)
                    .unwrap_or(&evidence.conversation_id)
                    .clone(),
            )
        }));
        ranked.push(record);
    }
    ranked
}

// The MVP centers on actionable preferences/corrections/decisions. Generic
// topics should not win an unknown-date tie merely through their hash value.
fn discovery_kind_priority(kind: ExtractionKind) -> u8 {
    match kind {
        ExtractionKind::ExplicitPreference => 5,
        ExtractionKind::Correction => 4,
        ExtractionKind::Decision => 3,
        ExtractionKind::ProjectReference => 2,
        ExtractionKind::InferredPreference | ExtractionKind::BehavioralPattern => 1,
        ExtractionKind::Topic => 0,
    }
}

fn set_evidence_counts(record: &mut ObservationRecord) {
    record.distinct_conversations = record
        .observation
        .evidence
        .iter()
        .map(|source| {
            (
                &source.provider,
                record
                    .conversation_origins
                    .get(&source.conversation_id)
                    .unwrap_or(&source.conversation_id),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    record.occurrences = record
        .observation
        .evidence
        .iter()
        .map(|source| {
            (
                &source.provider,
                record
                    .conversation_origins
                    .get(&source.conversation_id)
                    .unwrap_or(&source.conversation_id),
                record.occurrence_message(source),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
}

fn visible(status: ObservationStatus) -> bool {
    matches!(
        status,
        ObservationStatus::Candidate
            | ObservationStatus::Supported
            | ObservationStatus::UserConfirmed
    )
}

fn compare_time_desc(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (
        left.filter(|time| time.is_finite()),
        right.filter(|time| time.is_finite()),
    ) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn exact_snippet(content: &str) -> &str {
    let end = content
        .char_indices()
        .nth(SNIPPET_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(content.len());
    &content[..end]
}

fn source_date(timestamp: f64, timezone: Tz) -> Option<String> {
    if !timestamp.is_finite() {
        return None;
    }
    let seconds = timestamp.floor();
    if seconds < i64::MIN as f64 || seconds >= i64::MAX as f64 {
        return None;
    }
    let nanos = ((timestamp - seconds) * 1_000_000_000.0).floor() as u32;
    let utc = DateTime::<Utc>::from_timestamp(seconds as i64, nanos)?;
    // Bound before converting timezone so a hostile timestamp at Chrono's
    // representable extreme cannot overflow when applying a local UTC offset.
    if !(1..=9999).contains(&utc.year()) {
        return None;
    }
    let date = utc.with_timezone(&timezone).date_naive();
    if !(1..=9999).contains(&date.year()) {
        return None;
    }
    Some(date.format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        intelligence::{ExtractionKind, InferenceType, extract_rules},
        model::Conversation,
    };
    use serde_json::Value;

    fn target() -> AnalysisTarget {
        // 2026-10-18 22:30 UTC is already 2026-10-19 in Luxembourg.
        let before_midnight = DateTime::parse_from_rfc3339("2026-10-18T22:30:00Z")
            .unwrap()
            .timestamp() as f64;
        AnalysisTarget {
            revision: 1,
            conversation: Conversation {
                provider: "chatgpt".to_owned(),
                source_id: "project-observr".to_owned(),
                title: Some("Observr architecture".to_owned()),
                created_at: Some(before_midnight),
                updated_at: None,
                metadata: Value::Null,
                messages: vec![
                    message(
                        "user-before-midnight",
                        "user",
                        "I chose SQLite.",
                        Some(before_midnight),
                    ),
                    message(
                        "assistant",
                        "assistant",
                        "The implementation is done.",
                        Some(before_midnight + 60.0),
                    ),
                    message(
                        "user-after-midnight",
                        "user",
                        "Keep implementations simple.",
                        Some(before_midnight + 7200.0),
                    ),
                    message("unknown-user", "user", "Please deploy it.", None),
                    message("unknown-assistant", "assistant", "Okay.", None),
                ],
            },
        }
    }

    fn message(id: &str, role: &str, content: &str, created_at: Option<f64>) -> Message {
        Message {
            source_id: id.to_owned(),
            parent_source_id: None,
            role: role.to_owned(),
            content: content.to_owned(),
            created_at,
            metadata: Value::Null,
        }
    }

    fn observations(target: &AnalysisTarget) -> Vec<ObservationRecord> {
        extract_rules(&target.conversation, target.revision)
            .observations
            .into_iter()
            .map(|observation| ObservationRecord {
                id: observation.stable_id(),
                observation,
                status: ObservationStatus::Candidate,
                user_value: None,
                first_seen: None,
                last_seen: None,
                distinct_conversations: 999,
                occurrences: 999,
                feedback_count: 0,
                conversation_origins: BTreeMap::new(),
                message_origins: BTreeMap::new(),
            })
            .collect()
    }

    #[test]
    fn local_midnight_and_unknown_times_match_hand_labeled_dates() {
        let target = target();
        let records = observations(&target);
        let utc = summarize_days(std::slice::from_ref(&target), &records, "UTC").unwrap();
        assert_eq!(utc.unknown_time_messages, 2);
        assert_eq!(utc.unknown_time_user_messages, 1);
        assert_eq!(utc.days.len(), 2);
        assert_eq!(utc.days[0].date, "2026-10-18");
        assert_eq!(utc.days[1].date, "2026-10-19");
        assert_eq!(utc.days[0].user_message_count, 1);
        assert_eq!(utc.days[0].discussions[0].quote, "I chose SQLite.");
        assert_eq!(utc.days[0].observations.len(), 1);
        assert_eq!(
            utc.days[0].observations[0].observation.kind,
            ExtractionKind::Decision
        );
        let local =
            summarize_days(&[target.clone(), target], &records, "Europe/Luxembourg").unwrap();
        assert_eq!(local.days.len(), 1);
        assert_eq!(local.days[0].date, "2026-10-19");
        assert_eq!(local.days[0].user_message_count, 2);
        assert_eq!(local.days[0].conversation_count, 1);
        assert_eq!(local.days[0].observations.len(), 2);
        assert_eq!(local.unknown_time_messages, 2);
        assert_eq!(local.days[0].title, "Conversation activity digest");
    }

    #[test]
    fn evidence_is_exact_and_unknown_dates_are_not_substituted() {
        let target = target();
        let mut records = observations(&target);
        let mut fabricated = records[0].clone();
        fabricated.id = "fabricated".to_owned();
        fabricated.observation.evidence[0].quote = "Invented quotation".to_owned();
        records.push(fabricated);
        let mut stale = records[0].clone();
        stale.id = "stale".to_owned();
        stale.observation.evidence[0].revision = 2;
        records.push(stale);
        let report = summarize_days(std::slice::from_ref(&target), &records, "UTC").unwrap();
        assert_eq!(
            report
                .days
                .iter()
                .map(|day| day.observations.len())
                .sum::<usize>(),
            2
        );
        for day in report.days {
            for discussion in day.discussions {
                let source = target
                    .conversation
                    .messages
                    .iter()
                    .find(|message| message.source_id == discussion.source_message_id)
                    .unwrap();
                assert!(source.content.starts_with(&discussion.quote));
                assert_eq!(discussion.conversation_id, target.conversation.stable_id());
                assert_eq!(discussion.revision, 1);
                assert_eq!(discussion.provider, "chatgpt");
            }
            for record in day.observations {
                assert_eq!(record.distinct_conversations, 1);
                assert_eq!(record.occurrences, 1);
                assert_eq!(record.status, ObservationStatus::Candidate);
                assert_eq!(record.observation.inference, InferenceType::Explicit);
            }
        }
    }

    #[test]
    fn invalid_timezones_and_ambiguous_source_ids_fail_closed() {
        let target = target();
        assert!(summarize_days(std::slice::from_ref(&target), &[], "Luxembourg").is_err());
        let mut changed = target.clone();
        changed.conversation.messages[0].content = "Different content.".to_owned();
        assert!(summarize_days(&[target, changed], &[], "UTC").is_err());
    }

    #[test]
    fn nonfinite_and_out_of_range_timestamps_remain_unknown() {
        let mut target = target();
        for (index, time) in [f64::NAN, f64::INFINITY, 1e100, -1e100]
            .into_iter()
            .enumerate()
        {
            target.conversation.messages.push(message(
                &format!("invalid-{index}"),
                "user",
                "A request.",
                Some(time),
            ));
        }
        let report = summarize_days(&[target], &[], "UTC").unwrap();
        assert_eq!(report.unknown_time_messages, 6);
        assert_eq!(report.unknown_time_user_messages, 5);
    }

    #[test]
    fn snippets_are_verbatim_and_unicode_safe() {
        let text = "é🦀".repeat(500);
        let snippet = exact_snippet(&text);
        assert_eq!(snippet.chars().count(), SNIPPET_CHARS);
        assert!(text.starts_with(snippet));
        assert_eq!(exact_snippet("small"), "small");
    }

    #[test]
    fn discovery_ties_prioritize_explicit_insights_over_generic_topics() {
        let mut records = Vec::new();
        for (id, kind) in [
            ("a", ExtractionKind::Topic),
            ("b", ExtractionKind::ProjectReference),
            ("c", ExtractionKind::Decision),
            ("d", ExtractionKind::ExplicitPreference),
            ("e", ExtractionKind::Correction),
        ] {
            let mut record = observations(&target()).remove(0);
            record.id = id.into();
            record.observation.kind = kind;
            record.last_seen = None;
            records.push(record);
        }
        assert_eq!(
            rank_discoveries(records, 5)
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["d", "e", "c", "b", "a"]
        );
    }

    #[test]
    fn short_discovery_lists_cover_another_source_within_the_same_evidence_tier() {
        let mut records = Vec::new();
        for (id, source, time) in [
            ("a", "one", 4.0),
            ("b", "one", 3.0),
            ("c", "one", 2.0),
            ("d", "two", 1.0),
        ] {
            let mut record = observations(&target()).remove(0);
            record.id = id.into();
            record.observation.kind = ExtractionKind::ExplicitPreference;
            record.last_seen = Some(time);
            for evidence in &mut record.observation.evidence {
                evidence.conversation_id = source.into();
            }
            records.push(record);
        }
        let full = rank_discoveries(records.clone(), usize::MAX);
        assert_eq!(
            full.iter()
                .take(3)
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "d", "b"]
        );
        let ranked = rank_discoveries(records, 3);
        assert_eq!(
            ranked
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "d", "b"]
        );
        assert_eq!(ranked[1].last_seen, Some(1.0));
        assert_eq!(ranked[1].distinct_conversations, 1);
    }

    #[test]
    fn ranking_uses_conversations_confirmation_recency_and_filters_hidden() {
        let target = target();
        let base = observations(&target).remove(0);
        let mut older = base.clone();
        older.id = "older".to_owned();
        older.last_seen = Some(100.0);
        let mut newer = older.clone();
        newer.id = "newer".to_owned();
        newer.last_seen = Some(200.0);
        let mut confirmed = older.clone();
        confirmed.id = "confirmed".to_owned();
        confirmed.status = ObservationStatus::UserConfirmed;
        let mut recurring = older.clone();
        recurring.id = "recurring".to_owned();
        let mut second = recurring.observation.evidence[0].clone();
        second.conversation_id = "a-second-conversation".to_owned();
        recurring.observation.evidence.push(second);
        let mut rejected = recurring.clone();
        rejected.id = "rejected".to_owned();
        rejected.status = ObservationStatus::UserRejected;
        let ranked = rank_discoveries(vec![older, newer, confirmed, recurring, rejected], 3);
        assert_eq!(
            ranked
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["recurring", "confirmed", "newer"]
        );
    }
}
