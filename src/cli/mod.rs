mod args;
mod output;

use output::{display_observations, print_json, terminal};

use anyhow::{Context, Result, ensure};
use args::*;
use clap::Parser;
use remnary::{
    analysis::{AnalysisEngine, AnalysisProgress, analyze_with_progress},
    analysis_store::ObservationStatus,
    intelligence::ExtractionKind,
    knowledge::sync_vault,
    local_sources::{detect_local, read_source},
    model::digest,
    reports::{rank_discoveries, summarize_days},
    store::Store,
    vault::{export_markdown, snapshot_markdown},
};
use std::{io::IsTerminal, path::Path};

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Sources { detect: true, .. } = &cli.command {
        let base = directories::BaseDirs::new().context("cannot locate home directory")?;
        print_json(&detect_local(base.home_dir())?)?;
        return Ok(());
    }
    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => directories::ProjectDirs::from("", "", "remnary")
            .context("cannot locate application data; provide --data-dir")?
            .data_local_dir()
            .to_path_buf(),
    };
    if let Command::Key {
        command: KeyCommand::Create { output },
    } = &cli.command
    {
        remnary::security::SecretKey::generate()?.write_new(output)?;
        println!(
            "Created private key file at {}. Keep it separate from database backups.",
            output.display()
        );
        return Ok(());
    }
    if let Command::Restore { backup } = &cli.command {
        let restore_key = if cli.insecure_plaintext {
            None
        } else {
            Some(remnary::security::SecretKey::from_key_file(
                cli.key_file
                    .as_deref()
                    .context("restore requires --key-file with the original backup recovery key")?,
            )?)
        };
        Store::restore_new(&data_dir, backup, restore_key.as_ref())?;
        println!(
            "Restored canonical data to {}. Generate a new vault explicitly; original projection files remain independently owned.",
            data_dir.display()
        );
        return Ok(());
    }
    if let Command::EncryptCopy {
        output,
        new_key_file,
    } = &cli.command
    {
        ensure!(
            cli.insecure_plaintext,
            "encrypt-copy requires --insecure-plaintext to explicitly select the legacy source"
        );
        let new_key = remnary::security::SecretKey::from_key_file(new_key_file)?;
        Store::encrypt_plaintext_copy(&data_dir, output, &new_key)?;
        println!(
            "Encrypted copy saved to {}. Original archive preserved. Restore into a new directory with the new key, then verify before retiring old plaintext copies.",
            output.display()
        );
        return Ok(());
    }
    let key = if cli.insecure_plaintext {
        None
    } else {
        Some(remnary::security::resolve_key(
            &data_dir,
            cli.key_file.as_deref(),
            matches!(cli.command, Command::Init),
        )?)
    };
    if let Command::Init = &cli.command {
        if let Some(key) = &key {
            Store::init_encrypted(&data_dir, key)?;
        } else {
            Store::init(&data_dir)?;
        }
        println!("Initialized Remnary at {}", data_dir.display());
        return Ok(());
    }
    let mut store = if let Some(key) = &key {
        Store::open_encrypted(&data_dir, key)?
    } else {
        Store::open(&data_dir)?
    };
    match cli.command {
        Command::Restore { .. }
        | Command::EncryptCopy { .. }
        | Command::Init
        | Command::Key {
            command: KeyCommand::Create { .. },
        } => unreachable!(),
        Command::Doctor => print_json(&store.doctor()?)?,
        Command::Backup {
            output,
            backup_key_file,
        } => {
            ensure!(
                !cli.insecure_plaintext || backup_key_file.is_none(),
                "use encrypt-copy --new-key-file to convert a plaintext archive; ordinary backups retain its encryption mode"
            );
            let backup_key = backup_key_file
                .as_deref()
                .map(remnary::security::SecretKey::from_key_file)
                .transpose()?;
            store.backup_to(&output, backup_key.as_ref().or(key.as_ref()))?;
            let protection = if key.is_some() {
                "encrypted; recovery requires the matching key"
            } else {
                "plaintext development copy"
            };
            println!(
                "Backup saved to {} ({protection}). Markdown exports and independent copies are not part of this database backup.",
                output.display()
            );
        }
        Command::Key {
            command: KeyCommand::Export { output },
        } => {
            key.as_ref()
                .context("plaintext stores have no encryption key to export")?
                .write_new(&output)?;
            println!(
                "Recovery key exported to {}. Keep this file private and separate from backups.",
                output.display()
            );
        }
        Command::Web { port } => return remnary::web::serve_store(store, port),
        Command::Status => {
            let stats = store.stats()?;
            println!(
                "Data: {}\nConversations: {}\nCurrent messages: {}\nRetained conversation revisions: {}",
                data_dir.display(),
                stats.conversations,
                stats.messages,
                stats.revisions
            );
            println!(
                "Completed analysis runs: {}\nVisible observations: {}\nExcluded conversations: {}\nPending projection cleanup: {}",
                store.analysis_run_count()?,
                store.observations(false)?.len(),
                store.excluded_conversations()?.len(),
                store.pending_cleanup_count()?
            );
        }
        Command::Import { file, source } => {
            let summary = store.import(&read_source(&file, source)?)?;
            println!(
                "Imported {} new, {} updated, {} unchanged, {} skipped conversations.",
                summary.inserted, summary.updated, summary.unchanged, summary.skipped
            );
        }
        Command::ImportAppSnapshot { file } => {
            let conversation = remnary::app_snapshot::read_snapshot(&file)?;
            let summary = store.import(&[conversation])?;
            println!(
                "Imported partial app view: {} new, {} updated, {} unchanged, {} skipped conversations. Coverage and truncation remain recorded.",
                summary.inserted, summary.updated, summary.unchanged, summary.skipped
            );
        }
        Command::Search { query, limit } => {
            let hits = store.search(&query, limit.into())?;
            if hits.is_empty() {
                println!("No matching messages.");
            }
            for hit in hits {
                println!(
                    "{} [{}]\n  {}:{}@{}#{}\n  {}\n",
                    terminal(hit.title.as_deref().unwrap_or("Untitled conversation")),
                    terminal(&hit.role),
                    terminal(&hit.provider),
                    terminal(&hit.conversation_id),
                    hit.revision,
                    terminal(&hit.message_id),
                    terminal(&hit.excerpt)
                );
            }
        }
        Command::Analyze {
            engine,
            model,
            endpoint,
            replace_other_models,
            progress,
        } => {
            let engine = match engine {
                EngineChoice::Rules => {
                    ensure!(model.is_none(), "--model requires --engine ollama");
                    ensure!(
                        !replace_other_models,
                        "--replace-other-models requires --engine ollama"
                    );
                    AnalysisEngine::Rules
                }
                EngineChoice::Ollama => AnalysisEngine::Ollama {
                    endpoint: &endpoint,
                    model: model.as_deref().context(
                        "--engine ollama requires --model with an installed local model",
                    )?,
                },
            };
            let show_progress = progress
                || (std::io::stderr().is_terminal()
                    && matches!(&engine, AnalysisEngine::Ollama { .. }));
            let summary = analyze_with_progress(&mut store, engine, |event| {
                if show_progress {
                    match event {
                        AnalysisProgress::Conversation { current, total } => {
                            eprintln!("Conversation {current}/{total}")
                        }
                        AnalysisProgress::Window {
                            conversation,
                            current,
                            total,
                        } => eprintln!(
                            "  Conversation {conversation}: model window {current}/{total}"
                        ),
                        AnalysisProgress::CheckingEvidence {
                            conversation,
                            candidates,
                            reused_identity,
                        } => eprintln!(
                            "  Conversation {conversation}: checking evidence for {candidates} {} candidates",
                            if reused_identity {
                                "reused"
                            } else {
                                "generated"
                            }
                        ),
                        AnalysisProgress::MatchingClaims {
                            conversation,
                            candidates,
                        } => eprintln!(
                            "  Conversation {conversation}: matching {candidates} claims with existing findings"
                        ),
                        AnalysisProgress::Finished {
                            conversation,
                            success,
                        } => eprintln!(
                            "  Conversation {conversation}: {}",
                            if success {
                                "saved"
                            } else {
                                "failed; remains pending"
                            }
                        ),
                    }
                }
            })?;
            println!(
                "Analyzed {} conversations; persisted {} candidate extractions.",
                summary.processed, summary.observations
            );
            if summary.rejected_candidates > 0 {
                println!(
                    "Omitted {} unsupported or uncertain candidate claims after separate evidence verification.",
                    summary.rejected_candidates
                );
            }
            if summary.processed == 0 && summary.failures.is_empty() {
                println!("No pending conversations for this processor and model.");
            }
            for failure in &summary.failures {
                eprintln!("{}", terminal(failure));
            }
            ensure!(
                summary.failures.is_empty(),
                "{} conversations failed analysis and remain pending",
                summary.failures.len()
            );
            if replace_other_models {
                let processor = summary
                    .processor
                    .as_ref()
                    .expect("analysis processor resolved");
                let retired = store.select_model_results(processor)?;
                println!(
                    "Selected model results; retired {retired} other active result sets. Human review and historical runs are preserved."
                );
                report_cleanup(&mut store)?;
            }
        }
        Command::Discover { all, json, limit } => {
            let records = store.observations(all)?;
            let records = if all {
                records.into_iter().take(limit.into()).collect()
            } else {
                rank_discoveries(records, limit.into())
            };
            display_observations(&records, json)?;
        }
        Command::Patterns { json, topics } => {
            if topics {
                let threads = store.topic_threads()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&threads)?);
                } else if threads.is_empty() {
                    println!("No topics with evidence from multiple independent conversations.");
                } else {
                    println!(
                        "Related discussions; individual claims and contexts remain separate. This does not establish an enduring preference.\n"
                    );
                    for thread in threads {
                        println!(
                            "{} — {} independent conversations, {} cited messages",
                            terminal(&thread.topic),
                            thread.distinct_conversations,
                            thread.occurrences
                        );
                        for id in thread.observation_ids {
                            println!("  Finding: {id}");
                        }
                    }
                }
                return Ok(());
            }
            let records = store
                .observations(false)?
                .into_iter()
                .filter(|o| o.distinct_conversations >= 2)
                .collect();
            display_observations(&rank_discoveries(records, 1000), json)?;
        }
        Command::Profile { candidates, json } => {
            let records = store
                .observations(false)?
                .into_iter()
                .filter(|o| candidates || o.status == ObservationStatus::UserConfirmed)
                .filter(|o| {
                    matches!(
                        o.observation.kind,
                        ExtractionKind::ExplicitPreference
                            | ExtractionKind::InferredPreference
                            | ExtractionKind::Correction
                            | ExtractionKind::BehavioralPattern
                            | ExtractionKind::ProjectReference
                            | ExtractionKind::Topic
                    )
                })
                .collect::<Vec<_>>();
            display_observations(&records, json)?;
            if !json && !candidates {
                println!(
                    "Profile includes user-confirmed observations. Use --candidates to review proposals."
                );
            }
        }
        Command::Timeline { json } => {
            let snapshots = store.current_snapshots()?;
            let mut events = Vec::new();
            for observation in store
                .observations(false)?
                .into_iter()
                .filter(|o| o.observation.kind == ExtractionKind::Decision)
            {
                for evidence in &observation.observation.evidence {
                    let time = snapshots
                        .iter()
                        .find(|s| {
                            s.conversation.stable_id() == evidence.conversation_id
                                && s.revision == evidence.revision
                        })
                        .and_then(|s| {
                            s.conversation
                                .messages
                                .iter()
                                .find(|m| m.source_id == evidence.source_message_id)
                        })
                        .and_then(|m| m.created_at);
                    events.push(serde_json::json!({"occurred_at":time,"observation_id":observation.id,"event":observation.observation.decision_event,"context":observation.observation.context,"status":observation.status,"summary":observation.display_text(),"evidence":evidence}));
                }
            }
            events.sort_by(|a, b| {
                a["occurred_at"]
                    .as_f64()
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&b["occurred_at"].as_f64().unwrap_or(f64::INFINITY))
            });
            if json {
                print_json(&events)?;
            } else {
                if events.is_empty() {
                    println!("No decision events yet. Import and analyze conversations first.");
                }
                for event in events {
                    print_json(&event)?;
                }
            }
        }
        Command::Today {
            date,
            timezone,
            json,
        } => {
            let zone: chrono_tz::Tz = timezone
                .parse()
                .context("timezone must be an IANA name such as UTC or Europe/Luxembourg")?;
            let date = date.unwrap_or_else(|| {
                chrono::Utc::now()
                    .with_timezone(&zone)
                    .date_naive()
                    .to_string()
            });
            chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .context("date must be YYYY-MM-DD")?;
            let report = summarize_days(
                &store.current_snapshots()?,
                &store.observations(false)?,
                &timezone,
            )?;
            if let Some(day) = report.days.into_iter().find(|day| day.date == date) {
                if json {
                    print_json(&day)?;
                } else {
                    println!(
                        "Conversation activity digest — {date} ({timezone})\n{} conversations; {} user messages.",
                        day.conversation_count, day.user_message_count
                    );
                    for source in day.discussions {
                        println!(
                            "\n{} @{} #{}\n  {}",
                            source.conversation_id,
                            source.revision,
                            terminal(&source.source_message_id),
                            terminal(&source.quote)
                        );
                    }
                }
            } else if json {
                print_json(
                    &serde_json::json!({"date":date,"timezone":timezone,"conversation_count":0,"user_message_count":0,"discussions":[]}),
                )?;
            } else {
                println!("No dated user messages recorded for {date} ({timezone}).");
            }
            if !json && report.unknown_time_messages > 0 {
                println!(
                    "{} messages have unknown or unsupported timestamps and were not assigned a day.",
                    report.unknown_time_messages
                );
            }
        }
        Command::Feedback { id, action, text } => {
            store.feedback(&id, action.into(), text.as_deref())?;
            println!(
                "Feedback saved for {id}. Run `remnary vault sync` to refresh exported knowledge."
            );
        }
        Command::Sources { .. } => {
            let records=store.current_snapshots()?.into_iter().map(|s|serde_json::json!({"id":s.conversation.stable_id(),"provider":s.conversation.provider,"source_id":s.conversation.source_id,"title":s.conversation.title,"revision":s.revision,"messages":s.conversation.messages.len()})).collect::<Vec<_>>();
            print_json(&records)?;
        }
        Command::Show { id, revision } => {
            ensure!(revision.is_none_or(|r| r > 0), "revision must be positive");
            print_json(&store.snapshot(&id, revision)?)?;
        }
        Command::Export { output } => export_transcripts(&mut store, &output)?,
        Command::Vault {
            command: VaultCommand::ApplyFeedback { file, check },
        } => {
            let feedback = remnary::vault_feedback::read_feedback(&file)?;
            remnary::vault_feedback::check_feedback(&store, &feedback)?;
            if check {
                print_json(&feedback)?;
            } else {
                remnary::vault_feedback::apply_feedback(&mut store, &feedback)?;
                println!(
                    "Feedback imported. Run `remnary vault sync` to regenerate knowledge notes."
                );
            }
        }
        Command::Vault {
            command: VaultCommand::Sync { output, timezone },
        } => {
            let output = output.unwrap_or_else(|| data_dir.join("vault"));
            let summary = sync_vault(&mut store, &output, &timezone)?;
            println!(
                "Vault: {} new or updated, {} unchanged, {} conflicts at {}.",
                summary.written,
                summary.unchanged,
                summary.conflicts,
                output.display()
            );
            ensure!(
                summary.conflicts == 0,
                "edited generated sections or unmanaged files were preserved; review conflicts before retrying"
            );
        }
        Command::Exclude { id } => {
            store.exclude_conversation(&id, true)?;
            println!("Conversation {id} excluded.");
            report_cleanup(&mut store)?;
        }
        Command::Include { id } => {
            store.exclude_conversation(&id, false)?;
            println!(
                "Manual exclusion removed for {id}. Provider/project/directory/topic rules still apply."
            );
        }
        Command::Privacy { command } => match command {
            PrivacyCommand::List => print_json(&store.exclusion_rules()?)?,
            PrivacyCommand::Add { scope, value } => {
                let rule = remnary::exclusions::ExclusionRule::new(scope.into(), &value)?;
                let affected = store.add_exclusion_rule(&rule)?;
                println!(
                    "Exclusion {} saved; {affected} retained conversations newly excluded. Future matching imports will be skipped.",
                    rule.id
                );
                report_cleanup(&mut store)?;
            }
            PrivacyCommand::Remove { id } => {
                store.remove_exclusion_rule(&id)?;
                println!(
                    "Rule removed. Other rules and manual exclusions still apply. Regenerate projections explicitly."
                );
            }
        },
        Command::Delete { target } => {
            let summary = match target {
                DeleteTarget::Conversation { id } => store.delete_conversation(&id)?,
                DeleteTarget::Observation { id } => store.delete_observation(&id)?,
            };
            println!(
                "Canonical data deleted. {} generated projections cleaned; {} conflicts.",
                summary.removed, summary.conflicts
            );
            ensure!(
                summary.conflicts == 0,
                "edited projections were preserved and remain in cleanup; inspect them and run `remnary cleanup`"
            );
        }
        Command::Cleanup => report_cleanup(&mut store)?,
    }
    Ok(())
}
fn export_transcripts(store: &mut Store, output: &Path) -> Result<()> {
    let conversations = store.conversations()?;
    let summary = export_markdown(&conversations, output)?;
    for conversation in &conversations {
        let (filename, content) = snapshot_markdown(conversation)?;
        let path = output.join(filename);
        if std::fs::read(&path).ok().as_deref() == Some(content.as_bytes()) {
            store.register_managed_file(
                &path,
                &digest(content.as_bytes()),
                &[conversation.stable_id()],
                &[],
            )?;
        }
    }
    println!(
        "Exported {} new, {} unchanged snapshots to {}.",
        summary.written,
        summary.unchanged,
        output.display()
    );
    ensure!(
        summary.conflicts == 0,
        "{} existing snapshots have edits or incomplete writes; preserved them. Export to a new directory to regenerate",
        summary.conflicts
    );
    Ok(())
}
fn report_cleanup(store: &mut Store) -> Result<()> {
    let summary = store.process_cleanup()?;
    println!(
        "{} projections cleaned; {} conflicts remain.",
        summary.removed, summary.conflicts
    );
    ensure!(
        summary.conflicts == 0,
        "edited generated projections remain; inspect them before retrying cleanup"
    );
    Ok(())
}
