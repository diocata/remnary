use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use observr::{import::read_chatgpt, store::Store, vault::export_markdown};

#[derive(Parser)]
#[command(
    name = "observr",
    version,
    about = "Your AI conversations, stored and searched locally"
)]
struct Cli {
    /// Override the OS application data directory (use .observr for a project-local store).
    #[arg(long, global = true, value_name = "DIRECTORY")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a local SQLite store. No sources are scanned automatically.
    Init,
    /// Show counts of current conversations, messages and retained source revisions.
    Status,
    /// Import an explicitly selected ChatGPT conversations.json or export ZIP.
    Import { file: PathBuf },
    /// Search current message text (all words must match within a message).
    Search {
        query: String,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u16).range(1..=1000))]
        limit: u16,
    },
    /// Export immutable Markdown conversation snapshots into a chosen folder.
    Export { output: PathBuf },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = match cli.data_dir {
        Some(directory) => directory,
        None => directories::ProjectDirs::from("", "", "observr")
            .context("cannot locate an application data directory; provide --data-dir")?
            .data_local_dir()
            .to_path_buf(),
    };
    match cli.command {
        Command::Init => {
            Store::init(&data_dir)?;
            println!("Initialized Observr at {}", data_dir.display());
            println!("Storage: embedded SQLite. Import a file to add conversations.");
        }
        Command::Status => {
            let stats = Store::open(&data_dir)?.stats()?;
            println!("Data: {}", data_dir.display());
            println!("Conversations: {}", stats.conversations);
            println!("Current messages: {}", stats.messages);
            println!("Retained conversation revisions: {}", stats.revisions);
        }
        Command::Import { file } => {
            let mut store = Store::open(&data_dir)?;
            let conversations = read_chatgpt(&file)?;
            let summary = store.import(&conversations)?;
            println!(
                "Imported {} new, {} updated, {} unchanged conversations.",
                summary.inserted, summary.updated, summary.unchanged
            );
        }
        Command::Search { query, limit } => {
            let hits = Store::open(&data_dir)?.search(&query, limit.into())?;
            if hits.is_empty() {
                println!("No matching messages.");
            }
            for hit in hits {
                println!(
                    "{} [{}]",
                    hit.title.as_deref().unwrap_or("Untitled conversation"),
                    hit.role
                );
                println!(
                    "  {}:{}@{}#{}",
                    hit.provider, hit.conversation_id, hit.revision, hit.message_id
                );
                println!("  {}\n", hit.excerpt);
            }
        }
        Command::Export { output } => {
            let conversations = Store::open(&data_dir)?.conversations()?;
            let summary = export_markdown(&conversations, &output)?;
            println!(
                "Exported {} new, {} unchanged snapshots to {}.",
                summary.written,
                summary.unchanged,
                output.display()
            );
            if summary.conflicts > 0 {
                anyhow::bail!(
                    "{} existing snapshots have edits or incomplete writes; preserved them. Export to a new directory to regenerate",
                    summary.conflicts
                );
            }
        }
    }
    Ok(())
}
