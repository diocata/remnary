//! Incremental analysis orchestration. Archived content is untrusted data.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    analysis_batches::model_batches,
    analysis_store::Processor,
    intelligence::{
        AnalysisOutput, extract_rules, group_observations, model_output_schema,
        validate_model_output,
    },
    ollama::{analyze_pinned_json, model_identity},
    privacy::redact_conversation,
    store::Store,
};

#[derive(Debug, Default, Serialize)]
pub struct AnalysisSummary {
    pub processor: Option<Processor>,
    pub processed: usize,
    pub observations: usize,
    pub rejected_candidates: usize,
    pub failures: Vec<String>,
}

pub enum AnalysisEngine<'a> {
    Rules,
    Ollama { endpoint: &'a str, model: &'a str },
}

/// Payload-free progress for long local-model runs. No titles, message text,
/// quotes or source metadata are exposed through this callback.
#[derive(Debug, Clone, Copy)]
pub enum AnalysisProgress {
    Conversation {
        current: usize,
        total: usize,
    },
    Window {
        conversation: usize,
        current: usize,
        total: usize,
    },
    Finished {
        conversation: usize,
        success: bool,
    },
}

/// Completed conversations commit independently. A failed conversation remains
/// pending and can be retried without repeating successful work.
pub fn analyze(store: &mut Store, engine: AnalysisEngine<'_>) -> Result<AnalysisSummary> {
    analyze_with_progress(store, engine, |_| {})
}

pub fn analyze_with_progress(
    store: &mut Store,
    engine: AnalysisEngine<'_>,
    mut progress: impl FnMut(AnalysisProgress),
) -> Result<AnalysisSummary> {
    let identity = match &engine {
        AnalysisEngine::Rules => None,
        AnalysisEngine::Ollama { endpoint, model } => Some(model_identity(endpoint, model)?),
    };
    let processor = match &engine {
        AnalysisEngine::Rules => Processor::rules(),
        AnalysisEngine::Ollama { model, .. } => {
            Processor::ollama_resolved(model, identity.as_ref().expect("model identity resolved"))
        }
    };
    let targets = store.analysis_targets(&processor)?;
    let mut summary = AnalysisSummary {
        processor: Some(processor.clone()),
        ..AnalysisSummary::default()
    };
    let total = targets.len();
    for (index, target) in targets.into_iter().enumerate() {
        let current = index + 1;
        progress(AnalysisProgress::Conversation { current, total });
        let result = (|| -> Result<(usize, usize)> {
            let mut rejected_candidates = 0;
            let mut reconciliation: Option<crate::reconciliation::Receipt> = None;
            // No raw message metadata or obvious credentials cross the analysis
            // boundary, including when using deterministic local rules.
            let redacted = redact_conversation(&target.conversation)?;
            let output = match &engine {
                AnalysisEngine::Rules => extract_rules(&redacted, target.revision),
                AnalysisEngine::Ollama { endpoint, model } => {
                    let mut observations = Vec::new();
                    let batches = model_batches(&redacted, target.revision)?;
                    let windows = batches.len();
                    for (window, batch) in batches.into_iter().enumerate() {
                        progress(AnalysisProgress::Window {
                            conversation: current,
                            current: window + 1,
                            total: windows,
                        });
                        let json = analyze_pinned_json(
                            endpoint,
                            model,
                            identity.as_ref().expect("model identity resolved"),
                            &model_output_schema(),
                            &batch,
                            target.revision,
                        )
                        .context("generating a local observation window")?;
                        let output =
                            serde_json::from_value::<AnalysisOutput>(json).map_err(|_| {
                                anyhow::anyhow!(
                                    "model output does not match the strict observation schema"
                                )
                            })?;
                        // Validate against the window actually sent, then the
                        // original immutable source again before committing.
                        let grounded = validate_model_output(&batch, target.revision, output)?;
                        let (verified, rejected) = crate::verification::verify_observations(
                            endpoint,
                            model,
                            identity.as_ref().expect("model identity resolved"),
                            &batch,
                            grounded,
                        )
                        .context("verifying newly generated observations")?;
                        rejected_candidates += rejected;
                        let (reconciled, receipt) = crate::reconciliation::reconcile(
                            store,
                            endpoint,
                            model,
                            identity.as_ref().expect("model identity resolved"),
                            &batch,
                            verified.clone(),
                        )?;
                        let verified = if reconciled != verified {
                            // Recheck the existing canonical claim against only
                            // this bounded window's NEW supporting citations.
                            let changes = AnalysisOutput {
                                observations: verified
                                    .observations
                                    .iter()
                                    .zip(&reconciled.observations)
                                    .filter(|(old, new)| old != new)
                                    .map(|(_, new)| new.clone())
                                    .collect(),
                                ..AnalysisOutput::default()
                            };
                            let (supported, _) = crate::verification::verify_observations(
                                endpoint,
                                model,
                                identity.as_ref().expect("model identity resolved"),
                                &batch,
                                changes,
                            )
                            .context("verifying a proposed canonical identity")?;
                            crate::reconciliation::retain_supported_bindings(
                                verified, reconciled, supported,
                            )?
                        } else {
                            reconciled
                        };
                        if let Some(previous) = &mut reconciliation {
                            previous.merge(receipt)?;
                        } else {
                            reconciliation = Some(receipt);
                        }
                        observations.extend(verified.observations);
                    }
                    AnalysisOutput {
                        observations: group_observations(&observations)
                            .into_iter()
                            .map(|group| group.observation)
                            .collect(),
                        ..AnalysisOutput::default()
                    }
                }
            };
            // Source identity is unchanged by redaction. Quotes must still be
            // exact spans in the ORIGINAL revision, so a redaction placeholder
            // cannot become fabricated supporting evidence.
            let count = if let Some(receipt) = reconciliation {
                store.save_reconciled_analysis(&target, &processor, output, &receipt)?
            } else {
                store.save_analysis(&target, &processor, output)?
            };
            Ok((count, rejected_candidates))
        })();
        progress(AnalysisProgress::Finished {
            conversation: current,
            success: result.is_ok(),
        });
        match result {
            Ok((count, rejected)) => {
                summary.processed += 1;
                summary.observations += count;
                summary.rejected_candidates += rejected;
            }
            Err(error) => {
                // Source identifiers are hashes; error text is produced by our
                // validators/adapter and never contains source/model payloads.
                summary.failures.push(format!(
                    "{}: analysis failed: {error:#}",
                    target.conversation.stable_id()
                ));
            }
        }
    }
    Ok(summary)
}
