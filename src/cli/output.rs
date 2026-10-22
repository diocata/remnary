use anyhow::Result;
use remnary::analysis_store::ObservationRecord;

pub(super) fn display_observations(records: &[ObservationRecord], json: bool) -> Result<()> {
    if json {
        return print_json(&records);
    }
    if records.is_empty() {
        println!(
            "No matching observations. Import conversations and run `remnary analyze` to extract candidates."
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
pub(super) fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
pub(super) fn terminal(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            output.extend(ch.escape_default());
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn terminal_escapes_controls_but_preserves_unicode_and_layout() {
        assert_eq!(
            super::terminal("Rust Δ 🦀\n\t\x1b[31m\r\0"),
            "Rust Δ 🦀\n\t\\u{1b}[31m\\r\\u{0}"
        );
    }
}
