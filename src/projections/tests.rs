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
fn managed_section_parser_preserves_exact_outside_bytes_and_nested_source_markers() -> Result<()> {
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
fn legacy_managed_markers_upgrade_and_preserve_owner_annotations() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let mut store = populated(&temporary.path().join("data"))?;
    let output = temporary.path().join("vault");
    sync_vault(&mut store, &output, "UTC")?;
    let path = output.join("Home.md");
    let text = fs::read_to_string(&path)?
        .replace(GENERATED_START, "<!-- observr:generated:start -->\n")
        .replace(GENERATED_END, "<!-- observr:generated:end -->");
    fs::write(&path, format!("My introduction\n{text}My annotation\n"))?;
    let result = sync_vault(&mut store, &output, "UTC")?;
    assert_eq!(result.conflicts, 0);
    let text = fs::read_to_string(path)?;
    assert!(text.starts_with("My introduction\n"));
    assert!(text.ends_with("My annotation\n"));
    assert!(text.contains(GENERATED_START));
    assert!(!text.contains("<!-- observr:generated:"));
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
        output.join("Topics"),
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
