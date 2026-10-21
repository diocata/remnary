//! Managed, evidence-linked Markdown projections. SQLite remains authoritative.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    analysis_store::{ObservationRecord, ObservationStatus},
    intelligence::{EvidenceRef, ExtractionKind},
    model::digest,
    reports::{rank_discoveries, summarize_days},
    store::Store,
    vault::{message_anchor, quoted, snapshot_markdown},
};

pub const GENERATED_START: &str = "<!-- observr:generated:start -->\n";
pub const GENERATED_END: &str = "<!-- observr:generated:end -->";

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct VaultSummary {
    pub written: usize,
    pub unchanged: usize,
    pub conflicts: usize,
}

struct Note {
    path: PathBuf,
    body: String,
    conversation_ids: Vec<String>,
    observation_ids: Vec<String>,
}

enum Outcome {
    Written,
    Unchanged,
    Conflict,
}

impl VaultSummary {
    fn add(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Written => self.written += 1,
            Outcome::Unchanged => self.unchanged += 1,
            Outcome::Conflict => self.conflicts += 1,
        }
    }
}

/// Return exact outside bytes and generated body, excluding marker bytes.
/// First-start/last-end intentionally tolerates literal copies inside source
/// code fences. A previously recorded body hash must be checked before mutation.
pub(crate) fn split_managed_section(content: &str) -> Result<(&str, &str, &str)> {
    let start = content
        .find(GENERATED_START)
        .context("generated start marker missing")?;
    let body_start = start + GENERATED_START.len();
    let end = content
        .rfind(GENERATED_END)
        .context("generated end marker missing")?;
    ensure!(end >= body_start, "generated markers are out of order");
    Ok((
        &content[..start],
        &content[body_start..end],
        &content[end + GENERATED_END.len()..],
    ))
}

/// Generate a dedicated Obsidian-compatible knowledge folder. All source text,
/// including user corrections and model summaries, is rendered in literal code
/// fences. Preexisting unregistered files and edited generated bodies conflict.
/// User bytes outside the managed section are preserved exactly.
pub fn sync_vault(store: &mut Store, output: &Path, timezone: &str) -> Result<VaultSummary> {
    let _projection_guard = store.projection_lock()?;
    // Validate the requested timezone before writing any files.
    let snapshots = store.current_snapshots()?;
    let records = store.observations(false)?;
    let _ = summarize_days(&snapshots, &records, timezone)?;
    private_directory(output)?;
    let output = fs::canonicalize(output).context("resolve vault output directory")?;
    let mut summary = VaultSummary::default();
    let mut transcripts: BTreeMap<(String, i64), String> = BTreeMap::new();
    let mut source_messages = BTreeMap::new();
    for snapshot in &snapshots {
        let conversation = &snapshot.conversation;
        let id = conversation.stable_id();
        let (filename, body) = snapshot_markdown(conversation)?;
        let relative = format!("Transcripts/{filename}");
        let note = Note {
            path: output.join(&relative),
            body,
            conversation_ids: vec![id.clone()],
            observation_ids: Vec::new(),
        };
        let outcome = write_note(store, &note)?;
        summary.add(&outcome);
        if !matches!(outcome, Outcome::Conflict) {
            transcripts.insert((id.clone(), snapshot.revision), relative);
            for message in &conversation.messages {
                source_messages.insert(
                    (id.clone(), snapshot.revision, message.source_id.clone()),
                    (conversation.provider.as_str(), message),
                );
            }
        }
    }
    // A conflicting transcript cannot silently become supporting evidence.
    let records: Vec<_> = records
        .into_iter()
        .filter_map(|mut record| {
            record.observation.evidence.retain(|evidence| {
                source_messages
                    .get(&(
                        evidence.conversation_id.clone(),
                        evidence.revision,
                        evidence.source_message_id.clone(),
                    ))
                    .is_some_and(|(provider, message)| {
                        *provider == evidence.provider
                            && message.role.eq_ignore_ascii_case("user")
                            && !evidence.quote.trim().is_empty()
                            && message.content.contains(&evidence.quote)
                    })
            });
            (!record.observation.evidence.is_empty()).then_some(record)
        })
        .collect();
    let records = rank_discoveries(records, usize::MAX);
    let days = summarize_days(&snapshots, &records, timezone)?;
    let all_conversations = snapshots
        .iter()
        .map(|s| s.conversation.stable_id())
        .collect::<Vec<_>>();
    let all_observations = records
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    let mut notes = Vec::new();
    let mut home = String::from(
        "# Observr\n\nYour imported conversation activity and evidence-backed observations.\n\n[Profile](Profile.md) · [Discoveries](Discoveries.md)\n\n",
    );
    home.push_str(&format!("- Imported conversations: {}\n- Active observations with available evidence: {}\n- Messages with unknown time: {} ({} user messages)\n\n", snapshots.len(), records.len(), days.unknown_time_messages, days.unknown_time_user_messages));
    quoted(&mut home, &format!("Daily timezone: {}", days.timezone));
    home.push_str("## Daily activity\n\nThese digests quote recorded requests and decisions; they do not establish that requested actions were completed.\n\n");
    for day in days.days.iter().rev() {
        home.push_str(&format!(
            "- [{}](Daily/{}.md): {} user messages across {} conversations\n",
            day.date, day.date, day.user_message_count, day.conversation_count
        ));
    }
    home.push_str("\n## Conversation transcripts\n\n");
    for path in transcripts.values() {
        home.push_str(&format!("- [Conversation snapshot]({path})\n"));
    }
    notes.push(Note {
        path: output.join("Home.md"),
        body: home,
        conversation_ids: all_conversations.clone(),
        observation_ids: all_observations.clone(),
    });

    let mut profile = String::from(
        "# Profile\n\n## User-confirmed knowledge\n\nOnly observations explicitly confirmed or corrected by you appear here. Historical decision events remain events; they do not assert the current choice.\n\n",
    );
    let mut candidates = String::from(
        "## Candidates awaiting review\n\nThese are observations of recorded interactions, not established profile facts.\n\n",
    );
    let mut confirmed_count = 0;
    for record in &records {
        if record.status == ObservationStatus::UserConfirmed {
            confirmed_count += 1;
            append_observation(&mut profile, record, "", &transcripts)?;
        } else {
            candidates.push_str(&format!(
                "- [Candidate {}](Candidates/{}.md)\n",
                record.id, record.id
            ));
            let mut body = String::from(
                "# Candidate observation\n\nThis has not been confirmed by the user. Review its source evidence before treating it as a fact.\n\n",
            );
            append_observation(&mut body, record, "../", &transcripts)?;
            notes.push(observation_note(
                output.join(format!("Candidates/{}.md", record.id)),
                body,
                record,
            ));
        }
    }
    if confirmed_count == 0 {
        profile.push_str("No user-confirmed knowledge yet.\n\n");
    }
    profile.push_str(&candidates);
    notes.push(Note {
        path: output.join("Profile.md"),
        body: profile,
        conversation_ids: all_conversations.clone(),
        observation_ids: all_observations.clone(),
    });

    let mut discoveries = String::from(
        "# Discoveries\n\nRanked by distinct supporting conversations, user confirmation and source recency. Repetition supports a candidate; it does not prove a personal trait.\n\n",
    );
    for record in &records {
        append_observation(&mut discoveries, record, "", &transcripts)?;
        if record.distinct_conversations >= 2 {
            let mut body = String::from(
                "# Recurring observation\n\nRecorded in multiple distinct conversations. Counts refer to available source evidence.\n\n",
            );
            append_observation(&mut body, record, "../", &transcripts)?;
            notes.push(observation_note(
                output.join(format!("Patterns/{}.md", record.id)),
                body,
                record,
            ));
        }
        if record.observation.kind == ExtractionKind::Decision {
            let mut body = String::from(
                "# Decision history event\n\nA consideration or proposal is not a final decision. This event alone does not establish the current choice.\n\n",
            );
            append_observation(&mut body, record, "../", &transcripts)?;
            notes.push(observation_note(
                output.join(format!("Decisions/{}.md", record.id)),
                body,
                record,
            ));
        }
    }
    if records.is_empty() {
        discoveries.push_str("No active observations with available evidence. Import conversations and run analysis.\n");
    }
    notes.push(Note {
        path: output.join("Discoveries.md"),
        body: discoveries,
        conversation_ids: all_conversations,
        observation_ids: all_observations,
    });

    for day in days.days {
        let mut body = format!(
            "# {} — Conversation activity digest\n\n{} user messages across {} conversations.\n\n",
            day.date, day.user_message_count, day.conversation_count
        );
        quoted(&mut body, &format!("Timezone: {}", day.timezone));
        for discussion in &day.discussions {
            body.push_str("## Recorded user message\n\n");
            if let Some(title) = &discussion.title {
                quoted(&mut body, title);
            }
            quoted(&mut body, &discussion.quote);
            let evidence = EvidenceRef {
                provider: discussion.provider.clone(),
                conversation_id: discussion.conversation_id.clone(),
                revision: discussion.revision,
                source_message_id: discussion.source_message_id.clone(),
                quote: discussion.quote.clone(),
            };
            if let Some(link) = evidence_link(&evidence, "../", &transcripts) {
                body.push_str(&format!("[Source message]({link})\n\n"));
            } else {
                body.push_str(
                    "Source transcript unavailable because of a preserved file conflict.\n\n",
                );
            }
        }
        if !day.observations.is_empty() {
            body.push_str("## Observations with evidence on this date\n\n");
        }
        for record in &day.observations {
            append_observation(&mut body, record, "../", &transcripts)?;
        }
        notes.push(Note {
            path: output.join(format!("Daily/{}.md", day.date)),
            body,
            conversation_ids: day
                .discussions
                .iter()
                .map(|source| source.conversation_id.clone())
                .collect(),
            observation_ids: day
                .observations
                .iter()
                .map(|record| record.id.clone())
                .collect(),
        });
    }
    let planned: BTreeSet<_> = notes.iter().map(|note| note.path.clone()).collect();
    for note in notes {
        summary.add(&write_note(store, &note)?);
    }
    // Remove stale generated claims after rejection, correction, source changes
    // or loss of supporting evidence. Unregistered user notes are untouched.
    for directory in ["Candidates", "Patterns", "Decisions", "Daily"] {
        let directory = output.join(directory);
        if !directory.exists() {
            continue;
        }
        ensure!(
            fs::symlink_metadata(&directory)?.is_dir(),
            "vault subdirectory is not a real directory"
        );
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if planned.contains(&path) || path.extension().is_none_or(|value| value != "md") {
                continue;
            }
            if !fs::symlink_metadata(&path)?.file_type().is_file() {
                summary.conflicts += 1;
                continue;
            }
            if store.managed_note_state(&path)?.is_none() {
                continue;
            }
            let note = Note {
                path,
                body: String::from(
                    "# Inactive generated view\n\nThis view has no active observations or dated source activity.\n",
                ),
                conversation_ids: Vec::new(),
                observation_ids: Vec::new(),
            };
            summary.add(&write_note(store, &note)?);
        }
    }
    Ok(summary)
}

fn observation_note(path: PathBuf, body: String, record: &ObservationRecord) -> Note {
    Note {
        path,
        body,
        conversation_ids: record
            .observation
            .evidence
            .iter()
            .map(|evidence| evidence.conversation_id.clone())
            .collect(),
        observation_ids: vec![record.id.clone()],
    }
}

fn append_observation(
    body: &mut String,
    record: &ObservationRecord,
    prefix: &str,
    transcripts: &BTreeMap<(String, i64), String>,
) -> Result<()> {
    body.push_str(&format!("## Observation {}\n\n", record.id));
    quoted(body, record.display_text());
    body.push_str(&format!("- Status: `{}`\n- Classification: `{}` / `{}`\n- Supporting conversations: {}\n- Source message occurrences: {}\n\n",serde_json::to_value(record.status)?.as_str().context("status is string")?,serde_json::to_value(record.observation.kind)?.as_str().context("kind is string")?,serde_json::to_value(record.observation.inference)?.as_str().context("inference is string")?,record.distinct_conversations,record.occurrences));
    if !record.conversation_origins.is_empty() {
        body.push_str("Copied cumulative history is counted once; all original source links are retained.\n\n");
    }
    if let Some(event) = record.observation.decision_event {
        body.push_str(&format!(
            "Decision event: `{}`\n\n",
            serde_json::to_value(event)?
                .as_str()
                .context("event is string")?
        ));
    }
    if let Some(context) = &record.observation.context {
        body.push_str("Context:\n\n");
        quoted(body, context);
    }
    body.push_str("### Evidence\n\n");
    for evidence in &record.observation.evidence {
        let link = evidence_link(evidence, prefix, transcripts)
            .context("observation transcript was not verified")?;
        body.push_str(&format!(
            "[Source message]({link}) · revision {}\n\n",
            evidence.revision
        ));
        quoted(body, &evidence.quote);
    }
    Ok(())
}

fn evidence_link(
    evidence: &EvidenceRef,
    prefix: &str,
    transcripts: &BTreeMap<(String, i64), String>,
) -> Option<String> {
    transcripts
        .get(&(evidence.conversation_id.clone(), evidence.revision))
        .map(|path| {
            format!(
                "{prefix}{path}#{}",
                message_anchor(&evidence.source_message_id)
            )
        })
}

fn private_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "vault output directory cannot be a symlink or non-directory"
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(path)
                .context("create private vault directory")?;
        }
        Err(error) => return Err(error).context("inspect vault directory"),
    }
    Ok(())
}

fn write_note(store: &mut Store, note: &Note) -> Result<Outcome> {
    let parent = note.path.parent().context("note has no parent directory")?;
    private_directory(parent)?;
    let existing = match fs::symlink_metadata(&note.path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > 128 * 1024 * 1024
            {
                return Ok(Outcome::Conflict);
            }
            Some(fs::read(&note.path).context("read managed note")?)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("inspect managed note"),
    };
    let previous_hash = store.managed_note_state(&note.path)?;
    let mut body = note.body.clone();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let content = if let Some(existing) = &existing {
        let Some(previous_hash) = previous_hash else {
            return Ok(Outcome::Conflict);
        };
        let Ok(text) = std::str::from_utf8(existing) else {
            return Ok(Outcome::Conflict);
        };
        let Ok((prefix, old_body, suffix)) = split_managed_section(text) else {
            return Ok(Outcome::Conflict);
        };
        if digest(old_body.as_bytes()) != previous_hash {
            return Ok(Outcome::Conflict);
        }
        format!("{prefix}{GENERATED_START}{body}{GENERATED_END}{suffix}")
    } else {
        format!("{GENERATED_START}{body}{GENERATED_END}\n")
    };
    let conversations: Vec<_> = note
        .conversation_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let observations: Vec<_> = note
        .observation_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let content_hash = digest(content.as_bytes());
    let generated_hash = digest(body.as_bytes());
    let changed = existing.as_deref() != Some(content.as_bytes());
    if changed {
        // Record ownership and all source dependencies before private bytes
        // cross into the filesystem. A crash can leave a tracked digest
        // conflict, but never an untracked completed projection.
        store.reserve_managed_note(
            &note.path,
            &content_hash,
            &generated_hash,
            &conversations,
            &observations,
        )?;
        // A named temporary file can survive a process crash. Reserve its
        // deterministic, per-target path before writing any private bytes too.
        let stage = stage_path(&note.path, &content_hash);
        match fs::symlink_metadata(&stage) {
            Ok(metadata) => {
                if !metadata.file_type().is_file()
                    || metadata.len() > 128 * 1024 * 1024
                    || store.managed_note_state(&stage)?.as_deref() != Some(generated_hash.as_str())
                    || fs::read(&stage)? != content.as_bytes()
                {
                    return Ok(Outcome::Conflict);
                }
                store.reserve_managed_note(
                    &stage,
                    &content_hash,
                    &generated_hash,
                    &conversations,
                    &observations,
                )?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                store.reserve_managed_note(
                    &stage,
                    &content_hash,
                    &generated_hash,
                    &conversations,
                    &observations,
                )?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut staged = match options.open(&stage) {
                    Ok(file) => file,
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                        return Ok(Outcome::Conflict);
                    }
                    Err(error) => return Err(error).context("create tracked projection stage"),
                };
                staged.write_all(content.as_bytes())?;
                staged.sync_all()?;
            }
            Err(error) => return Err(error).context("inspect tracked projection stage"),
        }
        if let Some(previous) = &existing {
            let metadata = fs::symlink_metadata(&note.path)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || fs::read(&note.path)? != *previous
            {
                return Ok(Outcome::Conflict);
            }
            fs::rename(&stage, &note.path).context("atomically replace managed section")?;
        } else {
            // A same-directory hard link publishes all bytes atomically and
            // fails if another writer created the destination in the meantime.
            match fs::hard_link(&stage, &note.path) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    return Ok(Outcome::Conflict);
                }
                Err(error) => return Err(error).context("atomically create managed note"),
            }
            fs::remove_file(&stage).context("remove published projection stage")?;
        }
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
    }
    store.register_managed_note(
        &note.path,
        &content_hash,
        &generated_hash,
        &conversations,
        &observations,
    )?;
    Ok(if changed {
        Outcome::Written
    } else {
        Outcome::Unchanged
    })
}

fn stage_path(path: &Path, content_hash: &str) -> PathBuf {
    let identity = digest(
        &serde_json::to_vec(&(path.to_string_lossy().as_ref(), content_hash))
            .expect("projection stage identity serializes"),
    );
    path.with_file_name(format!(".observr-write-{identity}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::{AnalysisEngine, analyze},
        analysis_store::FeedbackAction,
    };

    fn populated(root: &Path) -> Result<Store> {
        let mut store = Store::init(root)?;
        let archive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatgpt.json");
        let conversations = crate::import::read_chatgpt(&archive)?;
        store.import(&conversations)?;
        let analysis = analyze(&mut store, AnalysisEngine::Rules)?;
        ensure!(analysis.failures.is_empty(), "fixture analysis failed");
        Ok(store)
    }

    #[test]
    fn managed_section_parser_preserves_exact_outside_bytes_and_nested_source_markers() -> Result<()>
    {
        let text = format!(
            "user prefix\r\n{GENERATED_START}literal {GENERATED_START}quote {GENERATED_END}\nactual body\n{GENERATED_END}\r\nuser suffix"
        );
        let (prefix, body, suffix) = split_managed_section(&text)?;
        assert_eq!(prefix, "user prefix\r\n");
        assert_eq!(suffix, "\r\nuser suffix");
        assert!(body.contains(GENERATED_START));
        assert!(body.ends_with("actual body\n"));
        assert!(split_managed_section("ordinary user note").is_err());
        Ok(())
    }

    #[test]
    fn sync_is_idempotent_and_preserves_user_annotations() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        assert!(sync_vault(&mut store, &output, "UTC")?.written > 0);
        let second = sync_vault(&mut store, &output, "UTC")?;
        assert_eq!(second.written, 0);
        assert_eq!(second.conflicts, 0);
        let profile = output.join("Profile.md");
        let original = fs::read_to_string(&profile)?;
        fs::write(&profile, format!("my prefix\r\n{original}my suffix\r\n"))?;
        let record = store.observations(false)?.remove(0);
        store.feedback(
            &record.id,
            FeedbackAction::Correct,
            Some("My reviewed correction."),
        )?;
        assert!(sync_vault(&mut store, &output, "UTC")?.written > 0);
        let updated = fs::read_to_string(profile)?;
        assert!(updated.starts_with("my prefix\r\n"));
        assert!(updated.ends_with("my suffix\r\n"));
        assert!(updated.contains("My reviewed correction."));
        Ok(())
    }

    #[test]
    fn conflicting_generated_edits_and_unregistered_files_are_preserved() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        fs::create_dir(&output)?;
        fs::write(output.join("Home.md"), "My own home note")?;
        assert!(sync_vault(&mut store, &output, "UTC")?.conflicts >= 1);
        assert_eq!(
            fs::read_to_string(output.join("Home.md"))?,
            "My own home note"
        );
        let profile = output.join("Profile.md");
        let edited = fs::read_to_string(&profile)?.replace("# Profile", "# My edited profile");
        fs::write(&profile, &edited)?;
        assert!(sync_vault(&mut store, &output, "UTC")?.conflicts >= 2);
        assert_eq!(fs::read_to_string(profile)?, edited);
        Ok(())
    }

    #[test]
    fn all_generated_evidence_links_resolve_to_exact_source_anchors() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        sync_vault(&mut store, &output, "UTC")?;
        let link_pattern = regex::Regex::new(r"\[Source message\]\(([^)]+)#(message-[a-f0-9]+)\)")?;
        let mut checked = 0;
        for directory in [
            output.clone(),
            output.join("Candidates"),
            output.join("Patterns"),
            output.join("Decisions"),
            output.join("Daily"),
        ] {
            if !directory.exists() {
                continue;
            }
            for entry in fs::read_dir(&directory)? {
                let path = entry?.path();
                if !path.is_file() {
                    continue;
                }
                let text = fs::read_to_string(&path)?;
                for matched in link_pattern.captures_iter(&text) {
                    let target = directory.join(&matched[1]);
                    let transcript = fs::read_to_string(target)?;
                    assert!(transcript.contains(&format!("<a id=\"{}\"></a>", &matched[2])));
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
        Ok(())
    }

    #[test]
    fn rejecting_an_observation_retires_its_generated_candidate_note() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        sync_vault(&mut store, &output, "UTC")?;
        let record = store.observations(false)?.remove(0);
        let path = output.join(format!("Candidates/{}.md", record.id));
        assert!(path.exists());
        let original = fs::read_to_string(&path)?;
        fs::write(&path, format!("{original}My annotations survive."))?;
        store.feedback(&record.id, FeedbackAction::Reject, None)?;
        sync_vault(&mut store, &output, "UTC")?;
        let text = fs::read_to_string(path)?;
        assert!(text.contains("Inactive generated view"));
        assert!(text.ends_with("My annotations survive."));
        assert!(!text.contains(record.display_text()));
        Ok(())
    }

    #[test]
    fn reserved_completed_stage_resumes_after_simulated_process_interruption() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        private_directory(&output)?;
        let conversation_id = store.current_snapshots()?[0].conversation.stable_id();
        let note = Note {
            path: output.join("Crash-recovery.md"),
            body: "# Private source projection\n".to_owned(),
            conversation_ids: vec![conversation_id],
            observation_ids: Vec::new(),
        };
        let content = format!("{GENERATED_START}{}{GENERATED_END}\n", note.body);
        let content_hash = digest(content.as_bytes());
        let generated_hash = digest(note.body.as_bytes());
        let stage = stage_path(&note.path, &content_hash);
        store.reserve_managed_note(
            &note.path,
            &content_hash,
            &generated_hash,
            &note.conversation_ids,
            &[],
        )?;
        store.reserve_managed_note(
            &stage,
            &content_hash,
            &generated_hash,
            &note.conversation_ids,
            &[],
        )?;
        fs::write(&stage, &content)?;
        // Simulate stopping after the durable staging write but before publish.
        assert!(!note.path.exists());
        assert!(matches!(write_note(&mut store, &note)?, Outcome::Written));
        assert_eq!(fs::read_to_string(&note.path)?, content);
        assert!(!stage.exists());
        Ok(())
    }

    #[test]
    fn deletion_discovers_reserved_private_files_without_final_registration() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut store = populated(&temporary.path().join("data"))?;
        let output = temporary.path().join("vault");
        private_directory(&output)?;
        let source_id = store.current_snapshots()?[0].conversation.stable_id();
        let final_path = output.join("Interrupted.md");
        let body = "# Private source quotation\n";
        let content = format!("{GENERATED_START}{body}{GENERATED_END}\n");
        let content_hash = digest(content.as_bytes());
        let generated_hash = digest(body.as_bytes());
        let stage = stage_path(&final_path, &content_hash);
        for path in [&final_path, &stage] {
            store.reserve_managed_note(
                path,
                &content_hash,
                &generated_hash,
                std::slice::from_ref(&source_id),
                &[],
            )?;
        }
        // One crash point leaves completed staged text; another leaves the
        // published final path without the final verified registration.
        fs::write(&stage, &content)?;
        fs::write(&final_path, &content)?;
        let cleanup = store.delete_conversation(&source_id)?;
        assert_eq!(cleanup.conflicts, 0);
        assert_eq!(store.pending_cleanup_count()?, 0);
        for path in [&final_path, &stage] {
            assert!(!path.exists() || !fs::read_to_string(path)?.contains(body));
        }
        Ok(())
    }
}
