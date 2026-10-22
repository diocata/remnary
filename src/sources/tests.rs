use super::*;
#[test]
fn provider_generated_user_role_records_cannot_become_personal_facts() -> Result<()> {
    let claude = parse_claude(&[
        json!({"type":"user","sessionId":"test","uuid":"notification","origin":{"kind":"task-notification"},"message":{"role":"user","content":"Do not buy an expensive product."}}),
        json!({"type":"user","sessionId":"test","uuid":"human","origin":{"kind":"human"},"message":{"role":"user","content":"I prefer Rust."}}),
    ],Path::new("synthetic.jsonl"))?.remove(0);
    assert_eq!(claude.messages[0].role, "agent");
    assert_eq!(
        crate::intelligence::extract_rules(&claude, 1)
            .observations
            .len(),
        1
    );
    let codex = parse_codex(&[
        json!({"type":"session_meta","payload":{"id":"test"}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<codex_internal_context source=\"goal\">\nDo not invent user preferences.\n</codex_internal_context>"}]}}),
    ],Path::new("synthetic.jsonl"))?.remove(0);
    assert_eq!(codex.messages[0].role, "system");
    assert!(
        crate::intelligence::extract_rules(&codex, 1)
            .observations
            .is_empty()
    );
    Ok(())
}
#[test]
fn kiro_provider_prompt_and_environment_are_not_human_preferences() -> Result<()> {
    let record = json!({"executionId":"test","chat":[
        {"role":"human","content":"<identity>\nYou are Kiro, an AI assistant.\n</identity>\nDo not add tests."},
        {"role":"human","content":"I prefer Rust.\n<EnvironmentContext>\nI prefer fabricated environment instructions.\n</EnvironmentContext>"}
    ]});
    let source = parse_kiro(&record, Path::new("synthetic.chat"))?.remove(0);
    assert_eq!(source.messages[0].role, "system");
    assert_eq!(source.messages[1].content, "I prefer Rust.");
    assert_eq!(source.messages[1].metadata, record["chat"][1]);
    let output = crate::intelligence::extract_rules(&source, 1);
    assert_eq!(output.observations.len(), 1);
    assert_eq!(output.observations[0].canonical_value, "prefer rust");
    Ok(())
}
#[test]
fn inherited_codex_metadata_cannot_reassign_child_messages_to_human_parent() -> Result<()> {
    let records = vec![
        json!({"type":"session_meta","payload":{"id":"child","source":{"subagent":{}},"thread_source":"subagent"}}),
        json!({"type":"session_meta","payload":{"id":"parent","source":"vscode","thread_source":"user"}}),
        json!({"type":"response_item","ordinal":4,"payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer fabricated agent requests."}]}}),
    ];
    let sources = parse_codex(&records, Path::new("child.jsonl"))?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "child");
    assert_eq!(sources[0].messages[0].role, "agent");
    assert!(
        crate::intelligence::extract_rules(&sources[0], 1)
            .observations
            .is_empty()
    );
    Ok(())
}

#[test]
fn kiro_directory_coalesces_exact_prefixes_but_refuses_divergent_snapshots() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let first = json!({"executionId":"test","metadata":{"startTime":1000,"endTime":2000},"chat":[{"role":"human","content":"I prefer Rust."}]});
    let mut second = first.clone();
    second["chat"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"bot","content":"Okay"}));
    second["metadata"]["endTime"] = json!(3000);
    second["metadata"]["startTime"] = json!(2500);
    fs::write(
        directory.path().join("first.chat"),
        serde_json::to_vec(&first)?,
    )?;
    fs::write(
        directory.path().join("second.chat"),
        serde_json::to_vec(&second)?,
    )?;
    let sources = read_source(directory.path(), SourceFormat::Kiro)?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].messages.len(), 2);
    assert_eq!(sources[0].updated_at, Some(3.0));
    assert_eq!(sources[0].created_at, Some(1.0));
    assert_eq!(
        sources[0].metadata["source_paths"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(read_source(directory.path(), SourceFormat::Kiro)?, sources);
    second["chat"][0]["content"] = json!("A different history");
    fs::write(
        directory.path().join("second.chat"),
        serde_json::to_vec(&second)?,
    )?;
    assert!(read_source(directory.path(), SourceFormat::Kiro).is_err());
    Ok(())
}
#[test]
fn claude_preserves_ids_times_and_keeps_tools_and_agent_requests_out_of_user_facts() -> Result<()> {
    let records = vec![
        json!({"type":"user","sessionId":"test","uuid":"m1","timestamp":"2026-10-19T10:00:00+02:00","message":{"role":"user","content":"I prefer Rust."}}),
        json!({"type":"user","sessionId":"test","uuid":"m2","parentUuid":"m1","isSidechain":true,"message":{"role":"user","content":"I prefer invented agent instructions."}}),
        json!({"type":"user","sessionId":"test","uuid":"m3","message":{"role":"user","content":[{"type":"tool_result","content":"I prefer tool output"}]}}),
    ];
    let source = parse_claude(&records, Path::new("synthetic.jsonl"))?.remove(0);
    assert_eq!(source.messages.len(), 3);
    assert_eq!(source.messages[0].created_at, Some(1792396800.0));
    assert_eq!(source.messages[1].role, "agent");
    assert_eq!(source.messages[2].role, "tool");
    assert_eq!(
        crate::intelligence::extract_rules(&source, 1)
            .observations
            .len(),
        1
    );
    Ok(())
}
#[test]
fn codex_ignores_duplicate_event_text_and_provider_instructions() -> Result<()> {
    let records = vec![
        json!({"type":"session_meta","payload":{"id":"test","timestamp":"2026-10-19T08:00:00Z","source":"cli"}}),
        json!({"type":"event_msg","payload":{"type":"user_message","message":"I prefer Rust."}}),
        json!({"type":"response_item","ordinal":1,"payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"I prefer fake instructions"}]}}),
        json!({"type":"response_item","ordinal":2,"timestamp":"2026-10-19T08:01:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"I prefer Rust."}]}}),
    ];
    let source = parse_codex(&records, Path::new("synthetic.jsonl"))?.remove(0);
    assert_eq!(source.messages.len(), 2);
    assert_eq!(source.messages[1].source_id, "ordinal-2");
    assert_eq!(
        crate::intelligence::extract_rules(&source, 1)
            .observations
            .len(),
        1
    );
    Ok(())
}
#[test]
fn kiro_retains_unknown_message_dates_and_stable_fallback_identities() -> Result<()> {
    let source = json!({"executionId":"test","metadata":{"startTime":1792396800000u64,"endTime":1792396860000u64},"chat":[{"role":"human","content":"Keep code simple."},{"role":"bot","content":"Okay"},{"role":"human","content":"Keep code simple."}]});
    let first = parse_kiro(&source, Path::new("synthetic.chat"))?.remove(0);
    assert_eq!(first.created_at, Some(1792396800.0));
    assert!(first.messages.iter().all(|m| m.created_at.is_none()));
    assert_ne!(first.messages[0].source_id, first.messages[2].source_id);
    let mut appended = source;
    appended["chat"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"bot","content":"New answer"}));
    let second = parse_kiro(&appended, Path::new("synthetic.chat"))?.remove(0);
    assert_eq!(first.messages, second.messages[..3]);
    Ok(())
}
#[test]
fn discovery_import_is_read_only_and_directory_scanning_ignores_symlinks() -> Result<()> {
    let home = tempfile::tempdir()?;
    let directory = home.path().join(".claude/projects/project");
    fs::create_dir_all(&directory)?;
    let record=json!({"type":"user","sessionId":"test","uuid":"m1","message":{"role":"user","content":"I prefer Rust."}}).to_string();
    let path = directory.join("test.jsonl");
    fs::write(&path, &record)?;
    fs::write(
        directory.join("unrelated.credentials"),
        "must never be read",
    )?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(&path, directory.join("duplicate.jsonl"))?;
    let detected = detect_local(home.path())?;
    assert_eq!(detected[0].files, 1);
    let before = fs::read(&path)?;
    let imported = read_source(&directory, SourceFormat::ClaudeCode)?;
    assert_eq!(imported.len(), 1);
    assert_eq!(fs::read(path)?, before);
    assert!(read_source(&directory, SourceFormat::Auto).is_err());
    Ok(())
}
#[test]
fn malformed_ids_and_conflicting_message_records_fail_instead_of_dropping_evidence() {
    assert!(parse_kiro(&json!({"chat":[]}), Path::new("synthetic.chat")).is_err());
    assert!(
        parse_claude(
            &[json!({"type":"user","message":{"content":"hello"}})],
            Path::new("synthetic.jsonl")
        )
        .is_err()
    );
    assert!(
        parse_codex(
            &[json!({"type":"response_item","payload":{"type":"message","role":"user"}})],
            Path::new("synthetic.jsonl")
        )
        .is_err()
    );
}
