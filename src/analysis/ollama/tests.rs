use super::*;
use crate::model::Message;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

fn conversation() -> Conversation {
    Conversation {
        provider: "synthetic".into(),
        source_id: "synthetic-conversation".into(),
        title: Some("Synthetic test".into()),
        created_at: None,
        updated_at: None,
        metadata: json!({"secret_metadata": "must not be sent"}),
        messages: vec![Message {
            source_id: "user-message-1".into(),
            parent_source_id: None,
            role: "user".into(),
            content: "For this project, I prefer Rust.".into(),
            created_at: None,
            metadata: json!({"secret_metadata": "must not be sent"}),
        }],
    }
}

fn schema() -> Value {
    json!({"type":"object","properties":{"observations":{"type":"array"}},"required":["observations"],"additionalProperties":false})
}

fn read_request(stream: &mut TcpStream) -> (String, Value) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut received = Vec::new();
    let body_start = loop {
        let mut buffer = [0; 1024];
        let size = stream.read(&mut buffer).unwrap();
        assert!(size > 0, "request closed before headers");
        received.extend_from_slice(&buffer[..size]);
        if let Some(position) = received.windows(4).position(|part| part == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = String::from_utf8_lossy(&received[..body_start]);
    let request_line = headers.lines().next().unwrap();
    let path = request_line.split(' ').nth(1).unwrap().to_owned();
    assert!(request_line.starts_with("POST ") || request_line.starts_with("GET "));
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while received.len() < body_start + length {
        let mut buffer = [0; 1024];
        let size = stream.read(&mut buffer).unwrap();
        assert!(size > 0, "request closed before body");
        received.extend_from_slice(&buffer[..size]);
    }
    (
        path,
        if length == 0 {
            Value::Null
        } else {
            serde_json::from_slice(&received[body_start..body_start + length]).unwrap()
        },
    )
}

fn local_metadata() -> Value {
    json!({"details":{"format":"gguf"},"model_info":{"general.architecture":"synthetic"}})
}

fn server(
    response: String,
) -> (
    String,
    mpsc::Receiver<Value>,
    thread::JoinHandle<TcpListener>,
) {
    responses_server(vec![
        ("/api/show", reply(&local_metadata().to_string())),
        ("/api/chat", response),
    ])
}

fn preflight_server(
    response: String,
) -> (
    String,
    mpsc::Receiver<Value>,
    thread::JoinHandle<TcpListener>,
) {
    responses_server(vec![("/api/show", response)])
}

fn responses_server(
    responses: Vec<(&'static str, String)>,
) -> (
    String,
    mpsc::Receiver<Value>,
    thread::JoinHandle<TcpListener>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        for (expected_path, response) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let (path, body) = read_request(&mut stream);
            assert_eq!(path, expected_path);
            sender.send(body).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
        listener
    });
    (endpoint, receiver, handle)
}

fn reply(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn envelope(content: &str) -> String {
    json!({"message":{"role":"assistant","content":content},"done":true,"done_reason":"stop"})
        .to_string()
}

#[test]
fn wire_protocol_preserves_claim_fields_and_derives_description() {
    let schema = observation_protocol_schema(&crate::intelligence::model_output_schema());
    let branches = schema
        .pointer("/properties/observations/items/anyOf")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(branches.len(), 7);
    assert!(
        branches
            .iter()
            .all(|branch| branch["properties"]["e_subject"]["const"] == "user")
    );
    let kinds = branches
        .iter()
        .map(|branch| branch["properties"]["a_kind"]["const"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        kinds,
        [
            "behavioral_pattern",
            "correction",
            "decision",
            "explicit_preference",
            "inferred_preference",
            "project_reference",
            "topic"
        ]
        .into_iter()
        .collect()
    );
    for branch in branches {
        let names = branch["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            OBSERVATION_FIELDS
                .iter()
                .map(|(_, wire)| *wire)
                .collect::<Vec<_>>()
        );
        assert!(
            branch["required"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| names.contains(&entry.as_str().unwrap()))
        );
    }
    let original = json!({"kind":"decision","inference":"explicit","decision_event":"reversal","evidence":[{"provider":"synthetic","conversation_id":"c","revision":1,"source_message_id":"m","quote":"Café 🦀"}],"subject":"user","topic":"technology","context":null,"canonical_value":"SQLite","summary":"Reversal: SQLite"});
    let wire = OBSERVATION_FIELDS
        .iter()
        .map(|(field, wire)| ((*wire).to_owned(), original[*field].clone()))
        .collect::<serde_json::Map<_, _>>();
    let mut output = json!({"schema_version":1,"observations":[wire.clone()]});
    decode_observation_protocol(&mut output).unwrap();
    assert_eq!(output["observations"][0], original);
    let mut proposed_description = wire.clone();
    proposed_description.insert("i_summary".into(), json!("An independently proposed fact"));
    for invalid in [
        Value::Object(proposed_description),
        original.clone(),
        json!({"a_kind":"decision"}),
    ] {
        let mut output = json!({"observations":[invalid]});
        assert!(decode_observation_protocol(&mut output).is_err());
    }
}

#[test]
fn thinking_models_receive_reasoning_budget_without_relaxing_validation() {
    let mut metadata = local_metadata();
    metadata["capabilities"] = json!(["completion", "thinking"]);
    let (endpoint, requests, handle) = responses_server(vec![
        ("/api/show", reply(&metadata.to_string())),
        ("/api/chat", reply(&envelope(r#"{"observations":[]}"#))),
    ]);
    analyze_json(&endpoint, "local-thinking", &schema(), &conversation(), 1).unwrap();
    requests.recv_timeout(Duration::from_secs(3)).unwrap();
    let body = requests.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.join().unwrap();
    assert_eq!(body["think"], true);
    assert_eq!(body["options"]["num_predict"], 8192);
    assert_eq!(body["truncate"], false);
    assert_eq!(body["format"], schema());
}

#[test]
fn qwen_generation_and_identity_matching_avoid_automatic_reasoning() {
    for (family, identity_matching) in [
        ("qwen35", false),
        ("qwen35", true),
        ("gemma4", false),
        ("gemma4", true),
    ] {
        let mut metadata = local_metadata();
        metadata["details"]["family"] = json!(family);
        metadata["capabilities"] = json!(["thinking"]);
        let (endpoint, requests, handle) = responses_server(vec![
            ("/api/show", reply(&metadata.to_string())),
            ("/api/chat", reply(&envelope(r#"{"observations":[]}"#))),
        ]);
        if identity_matching {
            structured_json(
                &endpoint,
                "local-model",
                "Check only cited evidence",
                &json!({"candidates":[]}),
                &schema(),
            )
            .unwrap();
        } else {
            analyze_json(&endpoint, "local-model", &schema(), &conversation(), 1).unwrap();
        }
        requests.recv_timeout(Duration::from_secs(3)).unwrap();
        let body = requests.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(body["think"], false);
        assert_eq!(body["truncate"], false);
        assert_eq!(body["format"], schema());
        assert_eq!(body["options"]["temperature"], 0);
        assert_eq!(body["options"]["num_predict"], 4096);
        assert!(body["options"].get("seed").is_none());
        handle.join().unwrap();
    }
}

#[test]
fn support_verification_prefers_advertised_reasoning_without_exposing_thoughts() {
    for (family, capable) in [("qwen35", false), ("qwen35", true), ("gemma4", true)] {
        let mut metadata = local_metadata();
        metadata["details"]["family"] = json!(family);
        metadata["capabilities"] = if capable {
            json!(["thinking"])
        } else {
            json!(["completion"])
        };
        let mut response: Value =
            serde_json::from_str(&envelope(r#"{"observations":[]}"#)).unwrap();
        response["message"]["thinking"] = json!("INTERNAL_REASONING_NOT_RETURNED");
        let (endpoint, requests, handle) = responses_server(vec![
            ("/api/show", reply(&metadata.to_string())),
            ("/api/chat", reply(&response.to_string())),
        ]);
        let result = verify_json(
            &endpoint,
            "local-model",
            "Check cited claims",
            &json!({"candidates":[]}),
            &schema(),
        )
        .unwrap();
        assert!(
            !result
                .to_string()
                .contains("INTERNAL_REASONING_NOT_RETURNED")
        );
        requests.recv_timeout(Duration::from_secs(3)).unwrap();
        let body = requests.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(body["think"], capable);
        assert_eq!(
            body["options"]["temperature"],
            if capable { json!(1.0) } else { json!(0) }
        );
        if capable {
            if family == "qwen35" {
                assert_eq!(body["options"]["presence_penalty"], 1.5);
            }
            assert_eq!(body["options"]["top_p"], 0.95);
            assert_eq!(
                body["options"]["top_k"],
                if family == "gemma4" { 64 } else { 20 }
            );
            assert_eq!(body["options"]["seed"], 42);
        } else {
            assert!(body["options"].get("seed").is_none());
        }
        assert_eq!(
            body["options"]["num_predict"],
            if capable { 8192 } else { 4096 }
        );
        assert_eq!(body["truncate"], false);
        assert_eq!(body["format"], schema());
        handle.join().unwrap();
    }
}

#[test]
fn structured_request_preserves_citations_and_keeps_archive_as_data() {
    let (endpoint, request, handle) = server(reply(&envelope(r#"{"observations":[]}"#)));
    let result = analyze_json(&endpoint, "qwen3:8b", &schema(), &conversation(), 7).unwrap();
    assert_eq!(result, json!({"observations":[]}));
    assert_eq!(
        request.recv_timeout(Duration::from_secs(3)).unwrap(),
        json!({"model":"qwen3:8b"})
    );
    let body = request.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.join().unwrap();
    assert_eq!(body["format"], schema());
    assert_eq!(body["stream"], false);
    assert_eq!(body["think"], false);
    assert_eq!(body["truncate"], false);
    assert_eq!(body["shift"], false);
    assert_eq!(body["options"]["temperature"], 0);
    assert_eq!(body["options"]["num_ctx"], 32768);
    assert_eq!(body["options"]["num_predict"], 4096);
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "user");
    let source: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(source["conversation_id"], conversation().stable_id());
    assert_eq!(source["revision"], 7);
    assert!(source.get("title").is_none());
    assert!(source.get("coverage").is_none());
    assert_eq!(source["messages"][0]["source_id"], "user-message-1");
    assert!(!body.to_string().contains("secret_metadata"));
}

#[test]
fn local_endpoints_normalize_only_chat_paths() {
    for endpoint in [
        "http://localhost:11434",
        "http://127.0.0.1:11434/api/",
        "http://[::1]:11434/api/chat/",
    ] {
        let (url, address) = local_endpoint(endpoint).unwrap();
        assert_eq!(url.path(), "/api/chat");
        assert!(address.ip().is_loopback());
    }
}

#[test]
fn remote_ambiguous_or_credentialed_endpoints_are_rejected() {
    for endpoint in [
        "https://ollama.com",
        "http://example.com:11434",
        "http://192.168.1.2:11434",
        "http://localhost.example.com:11434",
        "http://[::ffff:127.0.0.1]:11434",
        "http://user:password@localhost:11434",
        "http://localhost:11434/api/chat?target=remote",
        "http://localhost:11434/#fragment",
        "http://localhost:11434/custom",
        "http://localhost:0",
    ] {
        assert!(local_endpoint(endpoint).is_err(), "accepted {endpoint}");
    }
}

#[test]
fn cloud_models_are_rejected_before_any_request() {
    for model in [
        "gpt-oss:120b-cloud",
        "qwen3:cloud",
        "Qwen3-CLOUD",
        "",
        "https://example.com/model",
        "model\nsecret",
    ] {
        assert!(validate_model(model).is_err(), "accepted {model:?}");
    }
    validate_model("qwen3:8b").unwrap();
}

#[test]
fn remote_alias_metadata_is_rejected_before_conversation_request() {
    let mut aliases = vec![
        json!({"remote_host":"https://ollama.com","remote_model":"cloud-model"}),
        json!({"details":{"format":"gguf","remote_model":"cloud-model"},"model_info":{}}),
        json!({"details":{"format":"gguf"},"model_info":{"remote_host":"https://example.com"}}),
        json!({"details":{"format":"remote"},"model_info":{}}),
        json!({}),
        json!([]),
    ];
    let mut alias = local_metadata();
    alias["remote_host"] = json!("https://ollama.com");
    aliases.push(alias);
    for metadata in aliases {
        let (endpoint, request, handle) = preflight_server(reply(&metadata.to_string()));
        let error = analyze_json(
            &endpoint,
            "innocent-local-alias",
            &schema(),
            &conversation(),
            1,
        )
        .unwrap_err();
        assert!(error.to_string().contains("no conversation was sent"));
        assert_eq!(
            request.recv_timeout(Duration::from_secs(3)).unwrap(),
            json!({"model":"innocent-local-alias"})
        );
        let listener = handle.join().unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }
}

#[test]
fn remote_inference_metadata_in_chat_is_not_accepted() {
    let response = json!({
        "message":{"role":"assistant","content":"{}"},"done":true,
        "remote_host":"https://ollama.com","remote_model":"other-model"
    })
    .to_string();
    let (endpoint, request, handle) = server(reply(&response));
    let error = analyze_json(&endpoint, "local-alias", &schema(), &conversation(), 1).unwrap_err();
    assert!(error.to_string().contains("remote inference"));
    request.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.join().unwrap();
}

#[test]
fn localhost_resolution_is_pinned_and_can_reach_loopback_daemon() {
    let (endpoint, request, handle) = server(reply(&envelope(r#"{"observations":[]}"#)));
    let endpoint = endpoint.replace("127.0.0.1", "localhost");
    analyze_json(&endpoint, "local-model", &schema(), &conversation(), 1).unwrap();
    request.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.join().unwrap();
}

#[test]
fn redirect_is_not_followed() {
    let forbidden = TcpListener::bind("127.0.0.1:0").unwrap();
    forbidden.set_nonblocking(true).unwrap();
    let response = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/api/chat\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        forbidden.local_addr().unwrap()
    );
    for during_preflight in [true, false] {
        let (endpoint, request, handle) = if during_preflight {
            preflight_server(response.clone())
        } else {
            server(response.clone())
        };
        let error =
            analyze_json(&endpoint, "local-model", &schema(), &conversation(), 1).unwrap_err();
        assert!(error.to_string().contains("redirects are refused"));
        request.recv_timeout(Duration::from_secs(3)).unwrap();
        handle.join().unwrap();
        assert!(
            matches!(forbidden.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }
}

#[test]
fn invalid_json_prose_array_and_incomplete_output_are_rejected() {
    let bodies = [
        "not-json".to_owned(),
        envelope("Here is the result: {}"),
        envelope("[]"),
        json!({"message":{"role":"assistant","content":"{}"},"done":false}).to_string(),
        json!({"message":{"role":"assistant","content":"{}"},"done":true,"done_reason":"length"})
            .to_string(),
        json!({"message":{"role":"user","content":"{}"},"done":true}).to_string(),
    ];
    for body in bodies {
        let (endpoint, request, handle) = server(reply(&body));
        assert!(analyze_json(&endpoint, "local-model", &schema(), &conversation(), 1).is_err());
        request.recv_timeout(Duration::from_secs(3)).unwrap();
        handle.join().unwrap();
    }
}

#[test]
fn response_limit_applies_with_and_without_content_length() {
    let oversized = envelope(&json!({"value":"x".repeat(512)}).to_string());
    let responses = [
        reply(&oversized),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{oversized}"
        ),
    ];
    for response in responses {
        let (endpoint, request, handle) = server(response);
        let error = analyze_with_limits(
            &endpoint,
            "local-model",
            &schema(),
            &conversation(),
            1,
            Duration::from_secs(3),
            256,
        )
        .unwrap_err();
        assert!(error.to_string().contains("safety limit"));
        request.recv_timeout(Duration::from_secs(3)).unwrap();
        handle.join().unwrap();
    }
}

fn tags(digit: char) -> String {
    reply(
        &json!({"models":[{"name":"local-model:latest","digest":digit.to_string().repeat(64)}]})
            .to_string(),
    )
}

#[test]
fn manifest_resolution_requires_a_valid_installed_digest_and_daemon_version() {
    let (endpoint, requests, handle) = responses_server(vec![
        ("/api/tags", tags('a')),
        ("/api/version", reply(r#"{"version":"0.40.6"}"#)),
    ]);
    let identity = model_identity(&endpoint, "local-model").unwrap();
    assert_eq!(identity.digest, "a".repeat(64));
    assert_eq!(identity.daemon_version, "0.40.6");
    for _ in 0..2 {
        assert_eq!(
            requests.recv_timeout(Duration::from_secs(3)).unwrap(),
            Value::Null
        );
    }
    handle.join().unwrap();
    for invalid in ["missing", "", "sha256:abc"] {
        let response =
            reply(&json!({"models":[{"name":"local-model:latest","digest":invalid}]}).to_string());
        let (endpoint, _requests, handle) = responses_server(vec![
            ("/api/tags", response),
            ("/api/version", reply(r#"{"version":"0.40.6"}"#)),
        ]);
        assert!(model_identity(&endpoint, "local-model").is_err());
        handle.join().unwrap();
    }
}

#[test]
fn model_change_before_or_during_generation_rejects_the_result() {
    let identity = ModelIdentity {
        digest: "a".repeat(64),
        daemon_version: "0.40.6".into(),
    };
    for change_before in [true, false] {
        let mut responses = vec![
            ("/api/tags", tags(if change_before { 'b' } else { 'a' })),
            ("/api/version", reply(r#"{"version":"0.40.6"}"#)),
        ];
        if !change_before {
            responses.extend([
                ("/api/show", reply(&local_metadata().to_string())),
                ("/api/chat", reply(&envelope(r#"{"observations":[]}"#))),
                ("/api/tags", tags('b')),
                ("/api/version", reply(r#"{"version":"0.40.6"}"#)),
            ]);
        }
        let (endpoint, _requests, handle) = responses_server(responses);
        let error = analyze_pinned_json(
            &endpoint,
            "local-model",
            &identity,
            &schema(),
            &conversation(),
            1,
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed during"));
        handle.join().unwrap();
    }
}

#[test]
fn all_generation_has_a_longer_but_bounded_deadline() {
    for (thinking, delay_ms, succeeds) in
        [(false, 600, true), (true, 600, true), (false, 1000, false)]
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            let mut details = local_metadata();
            if thinking {
                details["capabilities"] = json!(["thinking"]);
            }
            stream
                .write_all(reply(&details.to_string()).as_bytes())
                .unwrap();
            drop(stream);
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            thread::sleep(Duration::from_millis(delay_ms));
            let _ = stream.write_all(reply(&envelope(r#"{"observations":[]}"#)).as_bytes());
        });
        let result = analyze_with_limits(
            &endpoint,
            "local-model",
            &schema(),
            &conversation(),
            1,
            Duration::from_millis(400),
            MAX_RESPONSE_BYTES,
        );
        if succeeds {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(result.unwrap_err().to_string().contains("timed out"));
        }
        handle.join().unwrap();
    }
}

#[test]
fn decoder_quotes_exclude_redaction_spans_and_keep_untouched_clauses() {
    let original = "Contact person@example.test; I prefer Rust.";
    let redacted = crate::privacy::redact_text(original);
    let choices = quote_choices(&redacted);
    assert!(choices.iter().any(|quote| quote == "I prefer Rust."));
    assert!(
        choices
            .iter()
            .all(|quote| original.contains(quote) && !quote.contains(crate::privacy::REDACTED))
    );
}

#[test]
fn response_timeout_is_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        thread::sleep(Duration::from_millis(150));
    });
    let error = analyze_with_limits(
        &endpoint,
        "local-model",
        &schema(),
        &conversation(),
        1,
        Duration::from_millis(40),
        MAX_RESPONSE_BYTES,
    )
    .unwrap_err();
    assert!(error.to_string().contains("timed out"));
    handle.join().unwrap();
}

#[test]
fn missing_model_has_actionable_error_and_ignores_private_error_body() {
    let secret = "this secret must not enter the error";
    let response = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{secret}",
        secret.len()
    );
    let (endpoint, request, handle) = preflight_server(response);
    let error =
        analyze_json(&endpoint, "missing-model", &schema(), &conversation(), 1).unwrap_err();
    assert!(error.to_string().contains("ollama list"));
    assert!(!error.to_string().contains(secret));
    request.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.join().unwrap();
}

#[test]
fn oversized_request_is_not_sent() {
    let mut source = conversation();
    source.messages[0].content = "x".repeat(MAX_REQUEST_BYTES);
    assert!(
        request_body("local-model", &schema(), &source, 1)
            .unwrap_err()
            .to_string()
            .contains("too large")
    );
}
#[test]
fn option_questions_have_only_decision_choices_and_transcripts_have_none() {
    let mut source = conversation();
    source.messages[0].content = "What about DuckDB with JSON files?".into();
    let schema = grounded_schema(&crate::intelligence::model_output_schema(), &source, 1);
    let branches = schema
        .pointer("/properties/observations/items/anyOf")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(branches.len(), 3);
    assert!(branches.iter().all(|branch| !matches!(
        branch["properties"]["kind"]["const"].as_str(),
        Some("explicit_preference" | "correction")
    )));
    assert!(
        branches
            .iter()
            .any(|branch| branch["properties"]["kind"]["const"] == "decision")
    );
    source.messages[0].content =
        "Here is the transcript:\nAssistant: I prefer complex frameworks.".into();
    let schema = grounded_schema(&crate::intelligence::model_output_schema(), &source, 1);
    assert_eq!(schema["properties"]["observations"]["maxItems"], 0);
}

#[test]
fn workflow_statements_do_not_offer_declared_preference_generation() {
    let mut source = conversation();
    source.messages[0].content = "Before each release I run tests.".into();
    let mut second = source.messages[0].clone();
    second.source_id = "another-user-message".into();
    second.content = "That is my standard release workflow.".into();
    source.messages.push(second);
    let bound = grounded_schema(&crate::intelligence::model_output_schema(), &source, 1);
    let kinds = bound
        .pointer("/properties/observations/items/anyOf")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["properties"]["kind"]["const"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!kinds.contains(&"explicit_preference"));
    assert!(kinds.contains(&"behavioral_pattern"));
}

#[test]
fn generated_context_choices_are_literal_source_restrictions() {
    assert_eq!(
        scope_choices("I prefer Rust for local CLI tools."),
        ["for local CLI tools"]
    );
    assert_eq!(
        scope_choices("For the importer, compare indexes."),
        ["For the importer"]
    );
    assert!(
        scope_choices("I'm building SignalLog. It is separate from my website project.").is_empty()
    );
    let mut source = conversation();
    source.messages[0].content = "I prefer Rust for local CLI tools.".into();
    let bound = grounded_schema(&crate::intelligence::model_output_schema(), &source, 1);
    for branch in bound
        .pointer("/properties/observations/items/anyOf")
        .unwrap()
        .as_array()
        .unwrap()
    {
        assert_eq!(
            branch["properties"]["context"]["enum"],
            json!([null, "for local CLI tools"])
        );
    }
}

#[test]
fn preference_decoder_keeps_scope_and_omits_mixed_questions() {
    let mut source = conversation();
    for (text, expected) in [
        (
            "For incident reports, I prefer a short numbered checklist.",
            Some("For incident reports, I prefer a short numbered checklist."),
        ),
        ("I like the dashboard, could we try a denser layout?", None),
    ] {
        source.messages[0].content = text.into();
        let bound = grounded_schema(&crate::intelligence::model_output_schema(), &source, 1);
        let preference = bound
            .pointer("/properties/observations/items/anyOf")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|branch| branch["properties"]["kind"]["const"] == "explicit_preference");
        if let Some(expected) = expected {
            let choices = preference
                .unwrap()
                .pointer("/properties/evidence/items/anyOf/0/properties/quote/enum")
                .unwrap()
                .as_array()
                .unwrap();
            assert_eq!(choices, &vec![json!(expected)]);
        } else {
            assert!(preference.is_none());
        }
    }
}

#[test]
fn decoding_choices_bind_each_quote_to_its_actual_message_and_revision() {
    let mut source = conversation();
    source.messages[0].content = "I prefer Rust. Keep the code simple.\nCafé 🦀".into();
    let schema = grounded_schema(&crate::intelligence::output_schema(), &source, 7);
    let alternatives = schema
        .pointer("/properties/observations/items/anyOf")
        .unwrap()
        .as_array()
        .unwrap();
    for item in alternatives {
        let citations = item
            .pointer("/properties/evidence/items/anyOf")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(citations.len(), 1);
        let properties = &citations[0]["properties"];
        assert_eq!(properties["provider"]["const"], source.provider);
        assert_eq!(properties["conversation_id"]["const"], source.stable_id());
        assert_eq!(properties["revision"]["const"], 7);
        assert_eq!(
            properties["source_message_id"]["const"],
            source.messages[0].source_id
        );
        let quotes = properties["quote"]["enum"].as_array().unwrap();
        assert!(quotes.iter().any(|q| q == "I prefer Rust."));
        assert!(
            quotes
                .iter()
                .all(|q| source.messages[0].content.contains(q.as_str().unwrap()))
        );
    }
    let long = "🦀 café! ".repeat(1500);
    assert!(
        quote_choices(&long)
            .iter()
            .all(|q| q.len() <= 1024 && long.contains(q))
    );
}
