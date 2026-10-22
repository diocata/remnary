use super::*;
#[test]
fn routine_file_task_cannot_escape_as_decision_but_mixed_choice_is_retained() {
    let output:AnalysisOutput=serde_json::from_value(json!({"schema_version":1,"observations":[{"kind":"decision","inference":"explicit","subject":"user","topic":"file creation","context":null,"canonical_value":"proposes a puzzle file","summary":"Proposes creating a puzzle file.","decision_event":"proposal","evidence":[{"provider":"synthetic","conversation_id":"one","revision":1,"source_message_id":"m","quote":"Create a new file with a puzzle game."}]}]})).unwrap();
    let (filtered, omitted) = filter_ineligible_claims(output.clone());
    assert!(filtered.observations.is_empty());
    assert_eq!(omitted, 1);
    for quote in [
        "Run the tests for this change.",
        "Please run those tests now.",
        "Summarize the previous messages into Markdown.",
        "Translate this response into Portuguese.",
        "Remove AcmeEditor from my laptop.",
        "Please uninstall PaperEdit from my computer.",
        "Install LumenEdit on my Mac.",
    ] {
        assert!(routine_command(quote), "{quote}");
    }
    for quote in [
        "Create a new file for the parser. I prefer Rust for this tool.",
        "I chose Rust instead of Python for LumenLog.",
        "Create a new file in Rust because I chose it over Python.",
        "I am building LumenLog, a JSONL browser.",
        "Remove PaperEdit from my computer—you installed it without asking.",
        "Remove AcmeEditor from my laptop because I prefer LumenEdit.",
    ] {
        assert!(!routine_command(quote), "{quote}");
    }
}
