//! Best-effort secret redaction at the analysis boundary.
//!
//! This recognizes common credential/contact formats; it cannot guarantee that all
//! secrets, identifying information, or sensitive personal data are removed.
//! Redacted copies are analysis input, never replacements for source evidence.
//! A quote containing a replacement must not be accepted as an exact quotation
//! from the original message. Validate evidence against the original revision.

use std::sync::LazyLock;

use anyhow::Result;
use regex::{Captures, Regex};
use serde_json::Value;

use crate::model::Conversation;

pub const REDACTED: &str = "[REDACTED]";

// Compile fixed, reviewed expressions once. These patterns have no lookaround
// or backreferences; Rust's regex engine bounds matching time in input size.
static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // Include truncated blocks through end-of-input: partial pasted keys
        // must not leak merely because the closing PEM delimiter is missing.
        r"(?s)-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----.*?(?:-----END (?:[A-Z0-9]+ )*PRIVATE KEY-----|\z)",
        // Userinfo may be percent encoded, and a token can be used as a URL
        // username without a password. Remove the entire credentialed URL.
        r#"(?i)\b[a-z][a-z0-9+.-]*://[^\s/?#@<>"'`]+@[^\s<>"'`]+"#,
        // Common vendor token families. Deliberately conservative minimums
        // reduce accidental matches with ordinary identifiers and prose.
        r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
        r"\bglpat-[A-Za-z0-9_-]{20,}\b",
        r"\bsk-(?:proj-|ant-(?:api[0-9]+-)?)?[A-Za-z0-9_-]{12,}\b",
        r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{12,}\b",
        r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b",
        r"\bAIza[A-Za-z0-9_-]{30,}\b",
        r"\b(?:hf|npm)_[A-Za-z0-9]{20,}\b",
        // JWTs can carry credentials even without an adjacent token label.
        r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
        r"(?i)\bbearer[ \t]+[A-Za-z0-9._~+/-]{12,}={0,2}",
        r"(?i)\bauthorization[ \t]*:[ \t]*basic[ \t]+[A-Za-z0-9+/]{8,}={0,2}",
        // Common contact details are unnecessary for personal-preference
        // extraction. International '+' form avoids treating ordinary prices,
        // dates and recipe quantities as phone numbers.
        r"\+[1-9][0-9]{0,2}(?:[ \t().-]*[0-9]){7,12}\b",
        r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("fixed secret-redaction expression is valid"))
    .collect()
});

static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?P<prefix>\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|auth[_-]?token|session[_-]?token|client[_-]?secret|password|passwd|pwd|secret|token|(?:aws[_-]?)?(?:secret[_-]?access[_-]?key|access[_-]?key[_-]?id)|private[_-]?key)["']?[ \t]*[:=][ \t]*)(?P<secret>\[REDACTED\]|"(?:\\.|[^"\\\r\n])*"|'(?:\\.|[^'\\\r\n])*'|[^\s,;}\]`]+)"#,
    )
    .expect("fixed credential-assignment expression is valid")
});

/// Replace recognized credentials/contact details while keeping surrounding text intact.
///
/// Matching is heuristic and intentionally does not claim complete secret or
/// personal-information detection. Unmatched text is preserved byte-for-byte.
pub fn redact_text(text: &str) -> String {
    let mut redacted = text.to_owned();
    for pattern in SECRET_PATTERNS.iter() {
        redacted = pattern.replace_all(&redacted, REDACTED).into_owned();
    }
    ASSIGNMENT
        .replace_all(&redacted, |captures: &Captures<'_>| {
            let prefix = &captures["prefix"];
            let secret = &captures["secret"];
            // Preserve an existing placeholder exactly, making repeated passes
            // idempotent and avoiding treating the placeholder as a secret.
            if secret == REDACTED || secret == "\"[REDACTED]\"" || secret == "'[REDACTED]'" {
                captures[0].to_owned()
            } else {
                format!("{prefix}{REDACTED}")
            }
        })
        .into_owned()
}

/// Whether the text contains a credential/contact recognized by the current rules.
/// False means no known pattern matched, not that the text is safe to disclose.
pub fn has_secret(text: &str) -> bool {
    redact_text(text) != text
}

/// Create analysis input without mutating the canonical source conversation.
///
/// Source identities, roles, branch parents and timestamps remain unchanged.
/// Raw metadata is removed completely: it may duplicate unredacted text or
/// contain attachment credentials that are irrelevant to extraction. Titles
/// are redacted too. The fallible interface allows later policy validation.
pub fn redact_conversation(conversation: &Conversation) -> Result<Conversation> {
    let mut redacted = conversation.clone();
    redacted.title = redacted.title.as_deref().map(redact_text);
    redacted.metadata = Value::Null;
    for message in &mut redacted.messages {
        message.content = redact_text(&message.content);
        message.metadata = Value::Null;
    }
    Ok(redacted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Message;
    use serde_json::json;

    #[test]
    fn ordinary_text_and_unchanged_exact_quotes_are_preserved() {
        let text = "I prefer Rust. Keep explanations concise.\nUse SQLite at https://sqlite.org/docs.html; public API documentation.\n日本語の文章も保持する。";
        assert_eq!(redact_text(text), text);
        assert!(!has_secret(text));
        let original = "I prefer Rust. My api_key=example-secret-value; keep the code simple.";
        let redacted = redact_text(original);
        assert_eq!(
            redacted,
            "I prefer Rust. My api_key=[REDACTED]; keep the code simple."
        );
        for quote in ["I prefer Rust.", "keep the code simple."] {
            assert!(original.contains(quote) && redacted.contains(quote));
        }
    }

    #[test]
    fn contact_minimization_preserves_ordinary_amounts_and_safe_clauses() {
        for contact in [
            "+1 (202) 555-0123",
            "+44 7700 900123",
            "person@example.test",
        ] {
            let text = format!("Contact {contact}; I prefer Rust.");
            assert_eq!(redact_text(&text), "Contact [REDACTED]; I prefer Rust.");
            assert!(has_secret(&text));
        }
        let ordinary = "Budget 300-700, use 250 grams, meet on 2026-10-20.";
        assert_eq!(redact_text(ordinary), ordinary);
    }

    #[test]
    fn vendor_tokens_are_replaced_without_leaving_secret_suffixes() {
        let secrets = [
            concat!("AKIA", "IOSFODNN7EXAMPLE"),
            concat!("ASIA", "IOSFODNN7EXAMPLE"),
            concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("gith", "ub_pat_11EXAMPLE_abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("glpa", "t-abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("sk-a", "bcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("sk-p", "roj-abcdefghijklmnopqrstuvwxyz0123456789_-ABC"),
            concat!("sk-a", "nt-api03-abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("sk_l", "ive_abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("xoxb", "-1234567890-abcdefghijklmnopqrstuvwxyz"),
            concat!("AIza", "SyABCDEFGHIJKLMNOPQRSTUVWXYZ0123456"),
            concat!("hf_a", "bcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("npm_", "abcdefghijklmnopqrstuvwxyz0123456789"),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJleGFtcGxlIn0.c3ludGhldGljLXNpZ25hdHVyZQ",
        ];
        for secret in secrets {
            let input = format!("before {secret} after");
            assert!(has_secret(&input), "missed synthetic token family");
            assert_eq!(redact_text(&input), "before [REDACTED] after");
        }
    }

    #[test]
    fn multiline_and_truncated_private_keys_are_redacted() {
        for kind in ["", "RSA ", "EC ", "ENCRYPTED ", "OPENSSH "] {
            let input = format!(
                "prefix\n-----BEGIN {kind}PRIVATE KEY-----\nsyntheticKeyData\nsecondLine\n-----END {kind}PRIVATE KEY-----\nsuffix"
            );
            assert_eq!(redact_text(&input), "prefix\n[REDACTED]\nsuffix");
        }
        assert_eq!(
            redact_text("prefix\n-----BEGIN RSA PRIVATE KEY-----\nsyntheticUnclosedData"),
            "prefix\n[REDACTED]"
        );
        let public = "-----BEGIN PUBLIC KEY-----\npublic data\n-----END PUBLIC KEY-----";
        assert_eq!(redact_text(public), public);
    }

    #[test]
    fn credentialed_urls_are_removed_and_ordinary_urls_remain() {
        for url in [
            "https://alice:synthetic-password@example.test/private?q=secret",
            "postgres://alice:synthetic-password@localhost/database",
            "redis://:synthetic-password@localhost:6379/0",
            "https://alice:p%40ss%3Aword@example.test/path",
            "https://synthetic-token@example.test/private",
        ] {
            assert_eq!(
                redact_text(&format!("before {url} after")),
                "before [REDACTED] after"
            );
        }
        let public = "https://example.test/path?topic=rust#intro";
        assert_eq!(redact_text(public), public);
    }

    #[test]
    fn assignments_and_authorization_cover_common_paste_formats() {
        for (input, expected) in [
            ("API_KEY=synthetic-value", "API_KEY=[REDACTED]"),
            ("password: 'two words'", "password: [REDACTED]"),
            (
                r#"{"client_secret": "synthetic value", "normal": 1}"#,
                r#"{"client_secret": [REDACTED], "normal": 1}"#,
            ),
            (
                r#"password="some \"quoted\" value"; tail"#,
                "password=[REDACTED]; tail",
            ),
            (
                "AWS_SECRET_ACCESS_KEY=synthetic/secret+value",
                "AWS_SECRET_ACCESS_KEY=[REDACTED]",
            ),
            (
                "AWS_ACCESS_KEY_ID=syntheticvalue",
                "AWS_ACCESS_KEY_ID=[REDACTED]",
            ),
            ("Authorization: Basic dXNlcjpwYXNzd29yZA==", "[REDACTED]"),
            (
                "Authorization: Bearer synthetic_token_123456",
                "Authorization: [REDACTED]",
            ),
        ] {
            assert_eq!(redact_text(input), expected);
        }
    }

    #[test]
    fn repeated_redaction_is_idempotent() {
        for input in [
            "API_KEY=synthetic-value",
            "password: '[REDACTED]'",
            "api_key=[REDACTED]",
            "Bearer synthetic_token_123456",
            concat!("sk-p", "roj-abcdefghijklmnopqrstuvwxyz0123456789"),
        ] {
            let once = redact_text(input);
            assert_eq!(redact_text(&once), once);
            assert!(!has_secret(&once));
        }
    }

    #[test]
    fn analysis_copy_removes_raw_metadata_and_preserves_source_identity() -> Result<()> {
        let original = Conversation {
            provider: "chatgpt".into(),
            source_id: "synthetic-conversation".into(),
            title: Some("Configuration api_key=synthetic-title-secret".into()),
            created_at: Some(1000.0),
            updated_at: Some(2000.0),
            metadata: json!({"raw_text":"metadata-only-secret", "attachment_url":"private"}),
            messages: vec![Message {
                source_id: "synthetic-message".into(),
                parent_source_id: Some("synthetic-parent".into()),
                role: "user".into(),
                content: "I prefer Rust. password=synthetic-message-secret".into(),
                created_at: Some(1500.0),
                metadata: json!({"content":{"parts":["metadata-only-message-secret"]}}),
            }],
        };
        let copy = redact_conversation(&original)?;
        assert_eq!(copy.stable_id(), original.stable_id());
        assert_eq!(copy.provider, original.provider);
        assert_eq!(copy.source_id, original.source_id);
        assert_eq!(copy.created_at, original.created_at);
        assert_eq!(copy.updated_at, original.updated_at);
        assert_eq!(copy.messages[0].source_id, original.messages[0].source_id);
        assert_eq!(
            copy.messages[0].parent_source_id,
            original.messages[0].parent_source_id
        );
        assert_eq!(copy.messages[0].created_at, original.messages[0].created_at);
        assert_eq!(copy.messages[0].role, original.messages[0].role);
        assert_eq!(copy.metadata, Value::Null);
        assert_eq!(copy.messages[0].metadata, Value::Null);
        assert!(copy.messages[0].content.starts_with("I prefer Rust."));
        let serialized = serde_json::to_string(&copy)?;
        for secret in [
            "synthetic-title-secret",
            "synthetic-message-secret",
            "metadata-only",
        ] {
            assert!(!serialized.contains(secret));
        }
        assert!(
            original.messages[0]
                .content
                .contains("synthetic-message-secret")
        );
        assert!(!original.metadata.is_null());
        Ok(())
    }
}
