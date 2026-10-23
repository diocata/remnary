use clap::{Parser, Subcommand, ValueEnum};
use remnary::{analysis_store::FeedbackAction, local_sources::SourceFormat};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "remnary",
    version,
    about = "Evidence-backed knowledge from your AI conversations"
)]
pub(super) struct Cli {
    /// Override the OS application data directory.
    #[arg(long, global = true, value_name = "DIRECTORY")]
    pub(super) data_dir: Option<PathBuf>,
    /// Unlock with a private recovery key file instead of macOS Keychain.
    #[arg(long, global = true, conflicts_with = "insecure_plaintext")]
    pub(super) key_file: Option<PathBuf>,
    /// Development only: explicitly use unencrypted storage.
    #[arg(long, global = true)]
    pub(super) insecure_plaintext: bool,
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
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
    /// Import selected exports, local sessions or persisted Codex desktop history.
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
        /// Related discussions sharing a topic, with individual claims preserved.
        #[arg(long)]
        topics: bool,
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
pub(super) enum PrivacyCommand {
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
pub(super) enum PrivacyScope {
    Provider,
    Project,
    Directory,
    Topic,
}
impl From<PrivacyScope> for remnary::exclusions::ExclusionScope {
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
pub(super) enum VaultCommand {
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
pub(super) enum KeyCommand {
    Create { output: PathBuf },
    Export { output: PathBuf },
}
#[derive(Subcommand)]
pub(super) enum DeleteTarget {
    Conversation { id: String },
    Observation { id: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum EngineChoice {
    Rules,
    Ollama,
}
#[derive(Clone, Copy, ValueEnum)]
pub(super) enum ReviewAction {
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
