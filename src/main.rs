use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use observr::{
    analysis::{AnalysisEngine, AnalysisProgress, analyze_with_progress},
    analysis_store::{FeedbackAction, ObservationRecord, ObservationStatus},
    intelligence::ExtractionKind,
    knowledge::sync_vault,
    local_sources::{SourceFormat, detect_local, read_source},
    model::digest,
    reports::{rank_discoveries, summarize_days},
    store::Store,
    vault::{export_markdown, snapshot_markdown},
};
use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "observr",
    version,
    about = "Evidence-backed knowledge from your AI conversations"
)]
struct Cli {
    /// Override the OS application data directory.
    #[arg(long, global = true, value_name = "DIRECTORY")]
    data_dir: Option<PathBuf>,
    /// Unlock with a private recovery key file instead of macOS Keychain.
    #[arg(long, global = true, conflicts_with = "insecure_plaintext")]
    key_file: Option<PathBuf>,
    /// Development only: explicitly use unencrypted storage.
    #[arg(long, global = true)]
    insecure_plaintext: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check database integrity and pending projection cleanup without content.
    Doctor,
    /// Copy a legacy plaintext archive into a new encrypted database file.
    EncryptCopy {
        output: PathBuf,
        #[arg(long)]
        new_key_file: PathBuf,
    },
    /// Create a consistent encrypted backup at a new filename.
    Backup {
        output: PathBuf,
        #[arg(long)]
        backup_key_file: Option<PathBuf>,
    },
    /// Restore into a new data directory using an explicit recovery key.
    Restore { backup: PathBuf },
    /// Create a private key file or export the current recovery key.
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    /// Initialize local storage. No sources are scanned.
    Init,
    /// Show imported data and completed analysis counts.
    Status,
    /// Import selected ChatGPT exports or local Claude Code, Codex or Kiro sessions.
    Import {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = SourceFormat::Auto)]
        source: SourceFormat,
    },
    /// Import a partial ChatGPT JSON view from Codex app; '-' reads one JSON line.
    ImportAppSnapshot { file: PathBuf },
    /// Search current message text; all words must match within a message.
    Search {
        query: String,
        #[arg(long,default_value_t=10,value_parser=clap::value_parser!(u16).range(1..=1000))]
        limit: u16,
    },
    /// Incrementally extract grounded candidates with rules or a local model.
    Analyze {
        #[arg(long,value_enum,default_value_t=EngineChoice::Rules)]
        engine: EngineChoice,
        /// Installed Ollama model. Run the daemon with cloud disabled.
        #[arg(long)]
        model: Option<String>,
        /// After a fully successful run, select this model and retire other
        /// Ollama automatic results for sources with this exact completed run.
        #[arg(long)]
        replace_other_models: bool,
        /// Write payload-free progress to stderr (automatic for interactive model runs).
        #[arg(long)]
        progress: bool,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        endpoint: String,
    },
    /// Review observations and their exact source evidence.
    Discover {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
        #[arg(long,default_value_t=10,value_parser=clap::value_parser!(u16).range(1..=1000))]
        limit: u16,
    },
    /// Review observations repeated across distinct conversations.
    Patterns {
        #[arg(long)]
        json: bool,
    },
    /// Show confirmed knowledge, with optional unconfirmed candidates.
    Profile {
        #[arg(long)]
        candidates: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show chronological decision evidence, preserving each event type.
    Timeline {
        #[arg(long)]
        json: bool,
    },
    /// Show an evidence-backed activity digest for a local calendar day.
    Today {
        #[arg(long)]
        date: Option<String>,
        #[arg(long, default_value = "UTC")]
        timezone: String,
        #[arg(long)]
        json: bool,
    },
    /// Record authoritative feedback; corrections change display text.
    Feedback {
        id: String,
        #[arg(value_enum)]
        action: ReviewAction,
        #[arg(long)]
        text: Option<String>,
    },
    /// List imported conversation identities and revisions.
    Sources {
        #[arg(long)]
        json: bool,
        /// Inventory known local session directories without reading message text.
        #[arg(long)]
        detect: bool,
    },
    /// Inspect the exact normalized revision cited as evidence.
    Show {
        id: String,
        #[arg(long)]
        revision: Option<i64>,
    },
    /// Export immutable Markdown transcript snapshots to a chosen folder.
    Export { output: PathBuf },
    /// Generate knowledge notes while preserving user annotations.
    Vault {
        #[command(subcommand)]
        command: VaultCommand,
    },
    /// Start the authenticated local dashboard; Ctrl-C stops it.
    Web {
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// Exclude a conversation from import, analysis, search and projections.
    Exclude { id: String },
    /// Include a previously excluded conversation again.
    Include { id: String },
    /// Manage persistent provider, project, directory and literal-topic exclusions.
    Privacy {
        #[command(subcommand)]
        command: PrivacyCommand,
    },
    /// Delete source data or an observation and its generated projections.
    Delete {
        #[command(subcommand)]
        target: DeleteTarget,
    },
    /// Retry pending projection cleanup; edited generated sections conflict.
    Cleanup,
}
#[derive(Subcommand)]
enum PrivacyCommand {
    /// Show persistent exclusion rules (retained source data is not printed).
    List,
    /// Add an exclusion for retained sources and future imports.
    Add {
        #[arg(value_enum)]
        scope: PrivacyScope,
        value: String,
    },
    /// Remove one rule by ID; manual/overlapping exclusions remain.
    Remove { id: String },
}
#[derive(Clone, Copy, clap::ValueEnum)]
enum PrivacyScope {
    Provider,
    Project,
    Directory,
    Topic,
}
impl From<PrivacyScope> for observr::exclusions::ExclusionScope {
    fn from(scope: PrivacyScope) -> Self {
        match scope {
            PrivacyScope::Provider => Self::Provider,
            PrivacyScope::Project => Self::Project,
            PrivacyScope::Directory => Self::Directory,
            PrivacyScope::Topic => Self::Topic,
        }
    }
}

#[derive(Subcommand)]
enum VaultCommand {
    /// Apply one user-authored review block outside generated Markdown.
    ApplyFeedback {
        file: PathBuf,
        /// Validate and preview the action without saving feedback.
        #[arg(long)]
        check: bool,
    },
    Sync {
        output: Option<PathBuf>,
        #[arg(long, default_value = "UTC")]
        timezone: String,
    },
}
#[derive(Subcommand)]
enum KeyCommand {
    Create { output: PathBuf },
    Export { output: PathBuf },
}
#[derive(Subcommand)]
enum DeleteTarget {
    Conversation { id: String },
    Observation { id: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EngineChoice {
    Rules,
    Ollama,
}
#[derive(Clone, Copy, ValueEnum)]
enum ReviewAction {
    Confirm,
    Reject,
    Correct,
    Dismiss,
    Supersede,
    Reset,
}
impl From<ReviewAction> for FeedbackAction {
    fn from(value: ReviewAction) -> Self {
        match value {
            ReviewAction::Confirm => Self::Confirm,
            ReviewAction::Reject => Self::Reject,
            ReviewAction::Correct => Self::Correct,
            ReviewAction::Dismiss => Self::Dismiss,
            ReviewAction::Supersede => Self::Supersede,
            ReviewAction::Reset => Self::Reset,
        }
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Sources { detect: true, .. } = &cli.command {
        let base = directories::BaseDirs::new().context("cannot locate home directory")?;
        print_json(&detect_local(base.home_dir())?)?;
        return Ok(());
    }
    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => directories::ProjectDirs::from("", "", "observr")
            .context("cannot locate application data; provide --data-dir")?
            .data_local_dir()
            .to_path_buf(),
    };
    if let Command::Key {
        command: KeyCommand::Create { output },
    } = &cli.command
    {
        observr::security::SecretKey::generate()?.write_new(output)?;
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
            Some(observr::security::SecretKey::from_key_file(
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
        let new_key = observr::security::SecretKey::from_key_file(new_key_file)?;
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
        Some(observr::security::resolve_key(
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
        println!("Initialized Observr at {}", data_dir.display());
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
                .map(observr::security::SecretKey::from_key_file)
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
        Command::Web { port } => return observr::web::serve_store(store, port),
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
            let conversation = observr::app_snapshot::read_snapshot(&file)?;
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
        Command::Patterns { json } => {
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
                "Feedback saved for {id}. Run `observr vault sync` to refresh exported knowledge."
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
            let feedback = observr::vault_feedback::read_feedback(&file)?;
            observr::vault_feedback::check_feedback(&store, &feedback)?;
            if check {
                print_json(&feedback)?;
            } else {
                observr::vault_feedback::apply_feedback(&mut store, &feedback)?;
                println!(
                    "Feedback imported. Run `observr vault sync` to regenerate knowledge notes."
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
                let rule = observr::exclusions::ExclusionRule::new(scope.into(), &value)?;
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
                "edited projections were preserved and remain in cleanup; inspect them and run `observr cleanup`"
            );
        }
        Command::Cleanup => report_cleanup(&mut store)?,
    }
    Ok(())
}
fn display_observations(records: &[ObservationRecord], json: bool) -> Result<()> {
    if json {
        return print_json(&records);
    }
    if records.is_empty() {
        println!(
            "No matching observations. Import conversations and run `observr analyze` to extract candidates."
        );
    }
    for observation in records {
        println!(
            "{}\n  {}\n  {:?} / {:?}; {} occurrences across {} conversations",
            observation.id,
            terminal(observation.display_text()),
            observation.status,
            observation.observation.inference,
            observation.occurrences,
            observation.distinct_conversations
        );
        for evidence in &observation.observation.evidence {
            println!(
                "  Evidence {}:{}@{}#{}\n    {}",
                terminal(&evidence.provider),
                evidence.conversation_id,
                evidence.revision,
                terminal(&evidence.source_message_id),
                terminal(&evidence.quote)
            );
        }
        println!();
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
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn terminal(text: &str) -> String {
    text.chars()
        .flat_map(|ch| {
            if ch.is_control() && ch != '\n' && ch != '\t' {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}
