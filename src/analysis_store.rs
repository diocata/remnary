use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{
    intelligence::{
        AnalysisOutput, CandidateObservation, EvidenceRef, RULES_VERSION, group_observations,
        validate_output,
    },
    model::{Conversation, digest},
    store::Store,
};

#[derive(Debug, Clone)]
pub struct AnalysisTarget {
    pub conversation: Conversation,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Processor {
    pub engine: String,
    pub version: String,
    pub model: String,
    pub prompt_version: String,
}

impl Processor {
    pub fn rules() -> Self {
        Self {
            engine: "rules".into(),
            version: RULES_VERSION.into(),
            model: "deterministic".into(),
            prompt_version: "1".into(),
        }
    }

    fn run_id(&self, target: &AnalysisTarget) -> Result<String> {
        Ok(digest(&serde_json::to_vec(&(
            target.conversation.stable_id(),
            target.revision,
            self,
        ))?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    Candidate,
    Supported,
    UserConfirmed,
    UserRejected,
    Superseded,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub id: String,
    pub observation: CandidateObservation,
    pub status: ObservationStatus,
    pub user_value: Option<String>,
    pub first_seen: Option<f64>,
    pub last_seen: Option<f64>,
    pub distinct_conversations: usize,
    pub occurrences: usize,
    pub feedback_count: usize,
}

impl ObservationRecord {
    pub fn display_text(&self) -> &str {
        self.user_value
            .as_deref()
            .unwrap_or(&self.observation.summary)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackAction {
    Confirm,
    Reject,
    Correct,
    Dismiss,
    Supersede,
    Reset,
}

impl Store {
    pub fn analysis_targets(&self, processor: &Processor) -> Result<Vec<AnalysisTarget>> {
        let mut targets = Vec::new();
        for target in self.current_snapshots()? {
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM analysis_runs WHERE id = ?1)",
                [processor.run_id(&target)?],
                |row| row.get(0),
            )?;
            if !exists {
                targets.push(target);
            }
        }
        Ok(targets)
    }

    /// Validate untrusted model output before opening a write transaction, then
    /// verify the source snapshot again inside the transaction before persisting.
    pub fn save_analysis(
        &mut self,
        target: &AnalysisTarget,
        processor: &Processor,
        output: AnalysisOutput,
    ) -> Result<usize> {
        let output = validate_output(&target.conversation, target.revision, output)?;
        let source_id = target.conversation.stable_id();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active: Option<(i64, String)> = transaction.query_row(
            "SELECT c.current_revision, r.fingerprint FROM conversations c JOIN conversation_revisions r
             ON r.conversation_id = c.id AND r.revision = c.current_revision
             WHERE c.id = ?1 AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)",
            [&source_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        let active = active.context("conversation was deleted or excluded while analysis ran")?;
        ensure!(
            active.0 == target.revision && active.1 == target.conversation.fingerprint()?,
            "conversation changed while analysis ran; rerun analysis on its current revision"
        );
        let run_id = processor.run_id(target)?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO analysis_runs (id, conversation_id, revision, engine, processor_version, model, prompt_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![run_id, source_id, target.revision, processor.engine, processor.version, processor.model, processor.prompt_version],
        )?;
        if inserted == 0 {
            transaction.commit()?;
            return Ok(0);
        }
        let groups = group_observations(&output.observations);
        let mut count = 0;
        for group in groups {
            let observation = group.observation;
            let id = observation.stable_id();
            let deleted: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM deleted_observations WHERE observation_id = ?1)",
                [&id],
                |row| row.get(0),
            )?;
            if deleted {
                continue;
            }
            count += 1;
            transaction.execute(
                "INSERT OR IGNORE INTO observations
                 (id, kind, inference, subject, topic, canonical_value, context, summary, decision_event)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![id, enum_name(&observation.kind)?, enum_name(&observation.inference)?, observation.subject,
                    observation.topic, observation.canonical_value, observation.context, observation.summary,
                    observation.decision_event.as_ref().map(enum_name).transpose()?],
            )?;
            transaction.execute(
                "INSERT INTO observation_extractions (analysis_run_id, observation_id, payload) VALUES (?1, ?2, ?3)",
                params![run_id, id, serde_json::to_string(&observation)?],
            )?;
            for evidence in observation.evidence {
                transaction.execute(
                    "INSERT OR IGNORE INTO evidence (observation_id, conversation_id, revision, message_id, quote)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, evidence.conversation_id, evidence.revision, evidence.source_message_id, evidence.quote],
                )?;
            }
            // Recurrence supports a candidate but never confirms an inference.
            // User feedback is persistent and cannot be changed by reanalysis.
            transaction.execute(
                "UPDATE observations SET status = CASE WHEN (
                   SELECT count(DISTINCT e.conversation_id) FROM evidence e
                   JOIN conversations c ON c.id = e.conversation_id AND c.current_revision = e.revision
                   WHERE e.observation_id = ?1 AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)
                 ) >= 2 THEN 'supported' ELSE 'candidate' END,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1 AND status IN ('candidate', 'supported')",
                [&id],
            )?;
        }
        transaction.commit()?;
        Ok(count)
    }

    /// Return grounded observations whose evidence is present in active source
    /// revisions. Old extractions remain in SQLite for provenance and history.
    pub fn observations(&self, include_hidden: bool) -> Result<Vec<ObservationRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, kind, inference, subject, topic, canonical_value, context, summary, decision_event,
             status, user_value, (SELECT count(*) FROM user_feedback f WHERE f.observation_id = o.id)
             FROM observations o ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        let mut observations = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let status_text: String = row.get(9)?;
            let status: ObservationStatus = enum_value(status_text)?;
            if !include_hidden
                && matches!(
                    status,
                    ObservationStatus::UserRejected
                        | ObservationStatus::Dismissed
                        | ObservationStatus::Superseded
                )
            {
                continue;
            }
            let mut evidence_statement = self.connection.prepare(
                "SELECT c.provider, e.conversation_id, e.revision, e.message_id, e.quote, m.created_at
                 FROM evidence e JOIN conversations c ON c.id = e.conversation_id AND c.current_revision = e.revision
                 JOIN messages m ON m.conversation_id = e.conversation_id AND m.revision = e.revision AND m.source_id = e.message_id
                 WHERE e.observation_id = ?1 AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)
                 ORDER BY m.created_at, e.conversation_id, e.message_id, e.quote",
            )?;
            let mut evidence_rows = evidence_statement.query([&id])?;
            let mut evidence = Vec::new();
            let mut times = Vec::<f64>::new();
            let mut conversations = BTreeSet::new();
            let mut occurrences = BTreeSet::new();
            while let Some(row) = evidence_rows.next()? {
                let reference = EvidenceRef {
                    provider: row.get(0)?,
                    conversation_id: row.get(1)?,
                    revision: row.get(2)?,
                    source_message_id: row.get(3)?,
                    quote: row.get(4)?,
                };
                if let Some(time) = row.get::<_, Option<f64>>(5)? {
                    times.push(time);
                }
                conversations.insert(reference.conversation_id.clone());
                occurrences.insert((
                    reference.conversation_id.clone(),
                    reference.source_message_id.clone(),
                ));
                evidence.push(reference);
            }
            if evidence.is_empty() {
                continue;
            }
            let decision: Option<String> = row.get(8)?;
            // A supported claim may lose current evidence after a new import.
            let status = if matches!(
                status,
                ObservationStatus::Candidate | ObservationStatus::Supported
            ) {
                if conversations.len() >= 2 {
                    ObservationStatus::Supported
                } else {
                    ObservationStatus::Candidate
                }
            } else {
                status
            };
            observations.push(ObservationRecord {
                id,
                status,
                user_value: row.get(10)?,
                feedback_count: row.get::<_, i64>(11)? as usize,
                first_seen: times.iter().copied().min_by(f64::total_cmp),
                last_seen: times.iter().copied().max_by(f64::total_cmp),
                distinct_conversations: conversations.len(),
                occurrences: occurrences.len(),
                observation: CandidateObservation {
                    kind: enum_value(row.get(1)?)?,
                    inference: enum_value(row.get(2)?)?,
                    subject: row.get(3)?,
                    topic: row.get(4)?,
                    canonical_value: row.get(5)?,
                    context: row.get(6)?,
                    summary: row.get(7)?,
                    decision_event: decision.map(enum_value).transpose()?,
                    evidence,
                },
            });
        }
        Ok(observations)
    }

    pub fn feedback(
        &mut self,
        observation_id: &str,
        action: FeedbackAction,
        value: Option<&str>,
    ) -> Result<()> {
        let _guard = self.projection_lock()?;
        ensure!(
            !matches!(action, FeedbackAction::Correct)
                || value.is_some_and(|v| !v.trim().is_empty()),
            "correct feedback requires nonempty --text"
        );
        ensure!(
            matches!(action, FeedbackAction::Correct) || value.is_none(),
            "--text is only valid for correct feedback"
        );
        ensure!(
            value.is_none_or(|value| value.len() <= 16_384),
            "feedback text exceeds 16 KiB"
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: String = transaction
            .query_row(
                "SELECT status FROM observations WHERE id = ?1",
                [observation_id],
                |row| row.get(0),
            )
            .optional()?
            .context("observation not found; use discover --all to list observation IDs")?;
        crate::privacy_store::enqueue_observation_files(
            &transaction,
            observation_id,
            "feedback_changed",
        )?;
        let status = match action {
            FeedbackAction::Confirm | FeedbackAction::Correct => "user_confirmed",
            FeedbackAction::Reject => "user_rejected",
            FeedbackAction::Dismiss => "dismissed",
            FeedbackAction::Supersede => "superseded",
            FeedbackAction::Reset => "candidate",
        };
        transaction.execute("INSERT INTO user_feedback (observation_id, action, value, previous_status) VALUES (?1, ?2, ?3, ?4)",
            params![observation_id, enum_name(&action)?, value, previous])?;
        transaction.execute(
            "UPDATE observations SET status = ?1, user_value = CASE WHEN ?2 IN ('correct', 'reset') THEN ?3 ELSE user_value END,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?4",
            params![status, enum_name(&action)?, value, observation_id],
        )?;
        transaction.commit()?;
        self.process_cleanup_unlocked()?;
        Ok(())
    }

    pub fn analysis_run_count(&self) -> Result<usize> {
        Ok(self
            .connection
            .query_row("SELECT count(*) FROM analysis_runs", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn current_snapshots(&self) -> Result<Vec<AnalysisTarget>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut snapshots = Vec::new();
        for conversation in self.conversations()? {
            let revision = transaction.query_row(
                "SELECT current_revision FROM conversations WHERE id = ?1",
                [conversation.stable_id()],
                |row| row.get(0),
            )?;
            snapshots.push(AnalysisTarget {
                conversation,
                revision,
            });
        }
        transaction.commit()?;
        Ok(snapshots)
    }

    /// Read exactly the retained revision cited by an observation. Excluded
    /// sources are unavailable through the normal review interface.
    pub fn snapshot(&self, id: &str, revision: Option<i64>) -> Result<Conversation> {
        let transaction = self.connection.unchecked_transaction()?;
        let header: (String, String, Option<String>, Option<f64>, Option<f64>, String, i64) = transaction.query_row(
            "SELECT c.provider, c.source_id, r.title, r.created_at, r.updated_at, r.metadata, r.revision
             FROM conversations c JOIN conversation_revisions r ON r.conversation_id = c.id
             AND r.revision = COALESCE(?2, c.current_revision)
             WHERE c.id = ?1 AND NOT EXISTS(SELECT 1 FROM excluded_sources x WHERE x.conversation_id = c.id)",
            params![id, revision], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        ).optional()?.context("conversation revision not found or source is excluded")?;
        let mut statement = transaction.prepare(
            "SELECT source_id, parent_source_id, role, content, created_at, metadata FROM messages
             WHERE conversation_id = ?1 AND revision = ?2 ORDER BY position",
        )?;
        let mut rows = statement.query(params![id, header.6])?;
        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            messages.push(crate::model::Message {
                source_id: row.get(0)?,
                parent_source_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                metadata: serde_json::from_str(&row.get::<_, String>(5)?)?,
            });
        }
        drop(rows);
        drop(statement);
        transaction.commit()?;
        Ok(Conversation {
            provider: header.0,
            source_id: header.1,
            title: header.2,
            created_at: header.3,
            updated_at: header.4,
            metadata: serde_json::from_str(&header.5)?,
            messages,
        })
    }
}

fn enum_name(value: &impl Serialize) -> Result<String> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .context("expected string enum")
}

fn enum_value<T: serde::de::DeserializeOwned>(value: String) -> Result<T> {
    Ok(serde_json::from_value(serde_json::Value::String(value))?)
}
