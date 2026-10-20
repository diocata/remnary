//! Incremental analysis orchestration. Archived content is untrusted data.

use anyhow::Result;
use serde::Serialize;

use crate::{
    analysis_batches::model_batches,
    analysis_store::Processor,
    intelligence::{
        AnalysisOutput, extract_rules, group_observations, output_schema, validate_output,
    },
    ollama::analyze_json,
    privacy::redact_conversation,
    store::Store,
};

#[derive(Debug, Default, Serialize)]
pub struct AnalysisSummary {
    pub processed: usize,
    pub observations: usize,
    pub failures: Vec<String>,
}

pub enum AnalysisEngine<'a> {
    Rules,
    Ollama { endpoint: &'a str, model: &'a str },
}

/// Completed conversations commit independently. A failed conversation remains
/// pending and can be retried without repeating successful work.
pub fn analyze(store: &mut Store, engine: AnalysisEngine<'_>) -> Result<AnalysisSummary> {
    let processor = match &engine {
        AnalysisEngine::Rules => Processor::rules(),
        AnalysisEngine::Ollama { model, .. } => Processor {
            engine: "ollama".into(),
            version: "3".into(),
            model: (*model).into(),
            prompt_version: "3".into(),
        },
    };
    let targets = store.analysis_targets(&processor)?;
    let mut summary = AnalysisSummary::default();
    for target in targets {
        let result = (|| -> Result<usize> {
            // No raw message metadata or obvious credentials cross the analysis
            // boundary, including when using deterministic local rules.
            let redacted = redact_conversation(&target.conversation)?;
            let output = match &engine {
                AnalysisEngine::Rules => extract_rules(&redacted, target.revision),
                AnalysisEngine::Ollama { endpoint, model } => {
                    let mut observations = Vec::new();
                    for batch in model_batches(&redacted)? {
                        let json = analyze_json(
                            endpoint,
                            model,
                            &output_schema(),
                            &batch,
                            target.revision,
                        )?;
                        let output =
                            serde_json::from_value::<AnalysisOutput>(json).map_err(|_| {
                                anyhow::anyhow!(
                                    "model output does not match the strict observation schema"
                                )
                            })?;
                        // Validate against the window actually sent, then the
                        // original immutable source again before committing.
                        observations
                            .extend(validate_output(&batch, target.revision, output)?.observations);
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
            store.save_analysis(&target, &processor, output)
        })();
        match result {
            Ok(count) => {
                summary.processed += 1;
                summary.observations += count;
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
