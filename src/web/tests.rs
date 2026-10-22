use super::*;
use crate::analysis_store::{ObservationStatus, Processor};
use crate::intelligence::extract_rules;
use crate::model::{Conversation, Message};
use reqwest::Url;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

struct TestServer {
    origin: String,
    token: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(data_dir: &Path) -> Self {
        let server = bind(0).unwrap();
        let address = server.server_addr().to_ip().unwrap();
        assert_eq!(address.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
        let session = Session::new(address).unwrap();
        let origin = session.origin.clone();
        let token = session.token.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let data_dir = data_dir.to_path_buf();
        let thread = thread::spawn(move || {
            let mut store = Store::open(&data_dir).unwrap();
            while !worker_stop.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    handle(request, &session, &mut store);
                }
            }
        });
        Self {
            origin,
            token,
            stop,
            thread: Some(thread),
        }
    }

    fn client(&self) -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }

    fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client()
            .get(format!("{}{path}", self.origin))
            .bearer_auth(&self.token)
    }

    fn post(&self, body: Value) -> reqwest::blocking::RequestBuilder {
        self.client()
            .post(format!("{}/api/feedback", self.origin))
            .bearer_auth(&self.token)
            .json(&body)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn seeded_store(data_dir: &Path) -> (String, String) {
    let mut store = Store::init(data_dir).unwrap();
    let conversation = Conversation {
        provider: "synthetic".into(),
        source_id: "source-one".into(),
        title: Some("<img src=x onerror=alert(1)>".into()),
        created_at: Some(1_780_000_000.0),
        updated_at: None,
        metadata: json!({"unrelated":"must not be sent"}),
        messages: vec![Message {
            source_id: "message-one".into(),
            parent_source_id: None,
            role: "user".into(),
            content: "I prefer Rust.".into(),
            created_at: Some(1_780_000_000.0),
            metadata: json!({"private_metadata":true}),
        }],
    };
    let id = conversation.stable_id();
    store.import(&[conversation]).unwrap();
    let processor = Processor::rules();
    for target in store.analysis_targets(&processor).unwrap() {
        let output = extract_rules(&target.conversation, target.revision);
        store.save_analysis(&target, &processor, output).unwrap();
    }
    let observation = store.observations(false).unwrap().remove(0).id;
    (id, observation)
}

#[test]
fn private_api_requires_bearer_and_rejects_foreign_origin_host_and_query_tokens() {
    let directory = tempfile::tempdir().unwrap();
    seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let unauthorized = server
        .client()
        .get(format!("{}/api/status", server.origin))
        .send()
        .unwrap();
    assert_eq!(unauthorized.status(), 401);
    assert!(!unauthorized.text().unwrap().contains("conversations"));
    assert_eq!(
        server
            .get("/api/status")
            .header("Origin", "https://other.example")
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        server
            .get("/api/status")
            .header("Origin", "null")
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        server
            .get("/api/status")
            .header("Host", "example.com")
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        server
            .get("/api/status?token=not-accepted")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .get("/api/status")
            .header("Origin", &server.origin)
            .send()
            .unwrap()
            .status(),
        200
    );
    let response = server.get("/api/status").send().unwrap();
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
    assert!(response.headers().get("set-cookie").is_none());
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
}

#[test]
fn startup_token_is_random_and_confined_to_fragment() {
    let address = "127.0.0.1:8123".parse().unwrap();
    let first = Session::new(address).unwrap();
    let second = Session::new(address).unwrap();
    assert_ne!(first.token, second.token);
    assert_eq!(first.token.len(), 64);
    let launch = Url::parse(&first.launch_url()).unwrap();
    assert!(launch.query().is_none());
    assert!(launch.fragment().unwrap().starts_with("token="));
    assert!(!constant_time_equal(
        first.token.as_bytes(),
        second.token.as_bytes()
    ));
    assert!(constant_time_equal(
        first.token.as_bytes(),
        first.token.as_bytes()
    ));
}

#[test]
fn feedback_requires_json_and_persists_user_review() {
    let directory = tempfile::tempdir().unwrap();
    let (_, id) = seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let response = server
        .client()
        .post(format!("{}/api/feedback", server.origin))
        .bearer_auth(&server.token)
        .body("{}")
        .send()
        .unwrap();
    assert_eq!(response.status(), 415);
    assert_eq!(server.get("/api/feedback").send().unwrap().status(), 405);
    assert_eq!(
        server
            .post(json!({"observation_id":id,"action":"confirm","extra":"not allowed"}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .post(json!({"observation_id":id,"action":"correct","value":"  "}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .post(json!({"observation_id":id,"action":"confirm"}))
            .send()
            .unwrap()
            .status(),
        200
    );
    let records = Store::open(directory.path())
        .unwrap()
        .observations(false)
        .unwrap();
    assert_eq!(records[0].status, ObservationStatus::UserConfirmed);
    assert_eq!(server.post(json!({"observation_id":id,"action":"correct","value":"Use Rust for this project only."})).send().unwrap().status(), 200);
    let store = Store::open(directory.path()).unwrap();
    let records = store.observations(false).unwrap();
    assert_eq!(
        records[0].user_value.as_deref(),
        Some("Use Rust for this project only.")
    );
    assert_eq!(records[0].feedback_count, 2);
}

#[test]
fn oversized_mutation_and_unknown_routes_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let response = server
        .client()
        .post(format!("{}/api/feedback", server.origin))
        .bearer_auth(&server.token)
        .header("Content-Type", "application/json")
        .body("x".repeat(MAX_BODY_BYTES as usize + 1))
        .send()
        .unwrap();
    assert_eq!(response.status(), 413);
    let response = server
        .client()
        .post(format!("{}/api/feedback", server.origin))
        .bearer_auth(&server.token)
        .header("Content-Type", "application/json")
        .body(reqwest::blocking::Body::new(Cursor::new(vec![
            b'x';
            MAX_BODY_BYTES
                as usize
                + 1
        ])))
        .send()
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(server.get("/api/unknown").send().unwrap().status(), 404);
    assert_eq!(server.get("/unknown").send().unwrap().status(), 404);
    assert_eq!(
        server
            .get("/api/sources?unknown=true")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .get("/api/search?q=rust&q=sqlite")
            .send()
            .unwrap()
            .status(),
        400
    );
}

#[test]
fn source_revision_and_literal_search_are_available_without_raw_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let (id, _) = seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let source: Value = server
        .get(&format!("/api/source?conversation_id={id}&revision=1"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(source["messages"][0]["content"], "I prefer Rust.");
    assert_eq!(source["revision"], 1);
    assert!(!source.to_string().contains("private_metadata"));
    assert!(!source.to_string().contains("unrelated"));
    let source: Value = server
        .get(&format!("/api/source?conversation_id={id}"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(source["revision"], 1);
    assert_eq!(
        server
            .get(&format!("/api/source?conversation_id={id}&revision=0"))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .get(&format!("/api/source?conversation_id={id}&revision=999"))
            .send()
            .unwrap()
            .status(),
        404
    );
    let hits: Value = server
        .get("/api/search?q=Rust")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(hits["hits"].as_array().unwrap().len(), 1);
    assert_eq!(hits["hits"][0]["conversation_id"], id);
    assert_eq!(
        server
            .get("/api/search?q=%22%20OR%20*")
            .send()
            .unwrap()
            .status(),
        200
    );
}

#[test]
fn untrusted_strings_are_json_data_and_static_ui_uses_text_nodes() {
    let directory = tempfile::tempdir().unwrap();
    let (_, observation_id) = seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let hostile = "<img src=x onerror=alert(1)>";
    assert_eq!(
        server
            .post(json!({"observation_id":observation_id,"action":"correct","value":hostile}))
            .send()
            .unwrap()
            .status(),
        200
    );
    let records = server.get("/api/observations").send().unwrap();
    assert!(
        records.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let records: Value = records.json().unwrap();
    assert_eq!(records["observations"][0]["user_value"], hostile);
    let html = server
        .client()
        .get(format!("{}/", server.origin))
        .send()
        .unwrap();
    assert_eq!(html.status(), 200);
    assert!(!html.text().unwrap().contains(hostile));
    // A deliberate guard against introducing HTML sinks into the bundled
    // renderer; runtime browser QA also exercises hostile source titles.
    assert!(SCRIPT.contains("element.textContent = String(text)"));
    assert!(!SCRIPT.contains(".innerHTML"));
    assert!(!SCRIPT.contains("insertAdjacentHTML"));
    assert!(!SCRIPT.contains("eval("));
    assert!(!SCRIPT.contains("https://"));
}

#[test]
fn excluded_sources_disappear_from_every_browser_read_path() {
    let directory = tempfile::tempdir().unwrap();
    let (id, observation_id) = seeded_store(directory.path());
    let server = TestServer::start(directory.path());
    let mut store = Store::open(directory.path()).unwrap();
    store.exclude_conversation(&id, true).unwrap();
    let sources: Value = server.get("/api/sources").send().unwrap().json().unwrap();
    assert!(sources["sources"].as_array().unwrap().is_empty());
    let records: Value = server
        .get("/api/observations?hidden=1")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(records["observations"].as_array().unwrap().is_empty());
    let hits: Value = server
        .get("/api/search?q=Rust")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(hits["hits"].as_array().unwrap().is_empty());
    assert_eq!(
        server
            .get(&format!("/api/source?conversation_id={id}&revision=1"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        server
            .post(json!({"observation_id":observation_id,"action":"confirm"}))
            .send()
            .unwrap()
            .status(),
        404
    );
}
