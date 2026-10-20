//! A private, loopback dashboard for reviewing personal conversation evidence.
//!
//! Design brief: AI power users need to understand and review findings, with
//! original evidence beside each claim. Cool paper (#f3f6fa), ink (#17263f),
//! slate (#65738b), white (#ffffff), indigo (#4e5bd9), and cyan (#279caf) form a
//! quiet research workspace. Georgia display, system sans body, and monospace
//! source IDs separate interpretation from provenance. The signature is a
//! connected claim/evidence split, not a wall of dashboard statistics. An early
//! card-grid idea was rejected because it hid the supporting quotation.
//!
//! Layout: navigation | observation list -> original evidence. Small screens
//! stack the panes. Static assets contain no user data and use no remote
//! dependencies. Every API request requires a random per-process bearer token;
//! the startup link carries it in a fragment, never a query or an access log.

use std::io::{Cursor, Read};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use anyhow::{Context, Result};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::analysis_store::FeedbackAction;
use crate::store::Store;

const MAX_BODY_BYTES: u64 = 64 * 1024;
const MAX_URL_BYTES: usize = 8192;
const HTML: &str = include_str!("../web/index.html");
const SCRIPT: &str = include_str!("../web/app.js");
const STYLE: &str = include_str!("../web/style.css");
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; font-src 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// Run the local dashboard until the process is stopped. A zero port chooses
/// an available port; the actual private URL is printed once at startup.
pub fn serve(data_dir: &Path, port: u16) -> Result<()> {
    serve_store(Store::open(data_dir)?, port)
}

/// Serve an already unlocked archive without retaining its passphrase or key
/// in the HTTP layer. The caller owns the unlock policy and password prompt.
pub fn serve_store(mut store: Store, port: u16) -> Result<()> {
    let server = bind(port)?;
    let address = server
        .server_addr()
        .to_ip()
        .context("reading dashboard address")?;
    let session = Session::new(address)?;
    println!("Open your private dashboard: {}", session.launch_url());
    println!("Keep this link private. Press Ctrl+C to stop the dashboard.");
    for request in server.incoming_requests() {
        handle(request, &session, &mut store);
    }
    Ok(())
}

fn bind(port: u16) -> Result<Server> {
    Server::http((Ipv4Addr::LOCALHOST, port)).map_err(|_| {
        anyhow::anyhow!("cannot bind local dashboard to 127.0.0.1:{port}; choose a different port")
    })
}

struct Session {
    host: String,
    origin: String,
    token: String,
}

impl Session {
    fn new(address: SocketAddr) -> Result<Self> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| {
            anyhow::anyhow!("secure randomness is unavailable; dashboard was not started")
        })?;
        Ok(Self {
            host: address.to_string(),
            origin: format!("http://{address}"),
            token: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        })
    }

    fn launch_url(&self) -> String {
        format!("{}/#token={}", self.origin, self.token)
    }
}

fn constant_time_equal(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }
    expected
        .iter()
        .zip(supplied)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    let mut headers = request
        .headers()
        .iter()
        .filter(|header| header.field.equiv(name));
    let value = headers.next()?.value.as_str();
    headers.next().is_none().then_some(value)
}

fn handle(mut request: Request, session: &Session, store: &mut Store) {
    let result = dispatch(&mut request, session, store);
    let response = match result {
        Ok(response) => response,
        Err(error) => json_response(error.status, json!({"error":error.message})),
    };
    // Disconnected clients are normal. Request URLs, tokens, source contents,
    // and error payloads must never enter an access log.
    let _ = request.respond(response);
}

struct ApiError {
    status: u16,
    message: &'static str,
}

type ApiResult<T> = std::result::Result<T, ApiError>;

fn error(status: u16, message: &'static str) -> ApiError {
    ApiError { status, message }
}

fn dispatch(
    request: &mut Request,
    session: &Session,
    store: &mut Store,
) -> ApiResult<Response<Cursor<Vec<u8>>>> {
    if header(request, "Host") != Some(session.host.as_str()) {
        return Err(error(
            403,
            "This dashboard accepts only its exact loopback address.",
        ));
    }
    let origins: Vec<_> = request
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Origin"))
        .collect();
    if !origins.is_empty() && (origins.len() != 1 || origins[0].value.as_str() != session.origin) {
        return Err(error(403, "Requests from another origin are refused."));
    }
    let target = request.url();
    if target.len() > MAX_URL_BYTES || !target.starts_with('/') || target.starts_with("//") {
        return Err(error(400, "Invalid request path."));
    }
    let url = Url::parse(&format!("{}{target}", session.origin))
        .map_err(|_| error(400, "Invalid request path."))?;
    if url.query_pairs().any(|(key, _)| {
        key.eq_ignore_ascii_case("token") || key.eq_ignore_ascii_case("authorization")
    }) {
        return Err(error(
            400,
            "Authentication belongs in the private link fragment, never a query.",
        ));
    }
    let path = url.path();
    if path.starts_with("/api/") {
        let authorization =
            header(request, "Authorization").and_then(|value| value.strip_prefix("Bearer "));
        if !authorization.is_some_and(|supplied| {
            constant_time_equal(session.token.as_bytes(), supplied.as_bytes())
        }) {
            return Err(error(
                401,
                "Open the private dashboard link printed in your terminal.",
            ));
        }
        return api(request, store, &url);
    }
    if *request.method() != Method::Get {
        return Err(error(405, "This route accepts GET requests only."));
    }
    if url.query().is_some() {
        return Err(error(400, "This route does not accept query parameters."));
    }
    match path {
        "/" => Ok(response(
            200,
            "text/html; charset=utf-8",
            HTML.as_bytes().to_vec(),
        )),
        "/app.js" => Ok(response(
            200,
            "text/javascript; charset=utf-8",
            SCRIPT.as_bytes().to_vec(),
        )),
        "/style.css" => Ok(response(
            200,
            "text/css; charset=utf-8",
            STYLE.as_bytes().to_vec(),
        )),
        _ => Err(error(404, "Route not found.")),
    }
}

fn api(
    request: &mut Request,
    store: &mut Store,
    url: &Url,
) -> ApiResult<Response<Cursor<Vec<u8>>>> {
    match (request.method().clone(), url.path()) {
        (Method::Get, "/api/status") => {
            no_query(url)?;
            let stats = store.stats().map_err(|_| database_error())?;
            let observations = store.observations(false).map_err(|_| database_error())?;
            Ok(json_response(
                200,
                json!({"conversations":stats.conversations,"messages":stats.messages,"revisions":stats.revisions,"observations":observations.len()}),
            ))
        }
        (Method::Get, "/api/observations") => {
            let hidden = optional_query(url, "hidden")?;
            if hidden
                .as_deref()
                .is_some_and(|value| value != "1" && value != "0")
            {
                return Err(error(400, "The hidden option must be 0 or 1."));
            }
            let observations = store
                .observations(hidden.as_deref() == Some("1"))
                .map_err(|_| database_error())?;
            Ok(json_response(200, json!({"observations":observations})))
        }
        (Method::Get, "/api/search") => {
            let query = optional_query(url, "q")?.unwrap_or_default();
            if query.trim().is_empty() || query.len() > 2048 {
                return Err(error(400, "Enter a search query of 1 to 2048 bytes."));
            }
            let hits = store.search(&query, 100).map_err(|_| database_error())?;
            let hits: Vec<_> = hits.into_iter().map(|hit| json!({
                "provider":hit.provider,"conversation_id":crate::model::digest(&serde_json::to_vec(&(&hit.provider, &hit.conversation_id)).expect("string tuples serialize")),"title":hit.title,"revision":hit.revision,
                "message_id":hit.message_id,"role":hit.role,"excerpt":hit.excerpt
            })).collect();
            Ok(json_response(200, json!({"hits":hits})))
        }
        (Method::Get, "/api/sources") => {
            no_query(url)?;
            let sources = store.current_snapshots().map_err(|_| database_error())?;
            let sources: Vec<_> = sources.into_iter().map(|target| {
                let source = target.conversation;
                json!({"id":source.stable_id(),"provider":source.provider,"title":source.title,
                    "messages":source.messages.len(),"created_at":source.created_at,"updated_at":source.updated_at,"revision":target.revision,
                    "partial_view":crate::app_snapshot::is_partial(&source.metadata)})
            }).collect();
            Ok(json_response(200, json!({"sources":sources})))
        }
        (Method::Get, "/api/source") => {
            let mut id = None;
            let mut revision = None;
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "conversation_id" if id.is_none() => id = Some(value.into_owned()),
                    "revision" if revision.is_none() => {
                        let value = value
                            .parse::<i64>()
                            .ok()
                            .filter(|value| *value > 0)
                            .ok_or_else(|| error(400, "Revision must be a positive integer."))?;
                        revision = Some(value);
                    }
                    _ => return Err(error(400, "Unknown or repeated source query parameter.")),
                }
            }
            let id = id
                .filter(|id| valid_id(id))
                .ok_or_else(|| error(400, "A valid conversation ID is required."))?;
            let revision = match revision {
                Some(revision) => revision,
                None => store
                    .current_snapshots()
                    .map_err(|_| database_error())?
                    .into_iter()
                    .find(|target| target.conversation.stable_id() == id)
                    .map(|target| target.revision)
                    .ok_or_else(|| {
                        error(404, "This source revision is unavailable or excluded.")
                    })?,
            };
            let source = store
                .snapshot(&id, Some(revision))
                .map_err(|_| error(404, "This source revision is unavailable or excluded."))?;
            let partial_view = crate::app_snapshot::is_partial(&source.metadata);
            let messages: Vec<_> = source.messages.into_iter().map(|message| json!({
                "source_id":message.source_id,"parent_source_id":message.parent_source_id,"role":message.role,"content":message.content,"created_at":message.created_at,
                "truncated":message.metadata.pointer("/app_item/truncated").and_then(Value::as_bool).unwrap_or(false)
            })).collect();
            Ok(json_response(
                200,
                json!({"id":id,"provider":source.provider,"title":source.title,"revision":revision,"messages":messages,"partial_view":partial_view}),
            ))
        }
        (Method::Post, "/api/feedback") => {
            no_query(url)?;
            let content_type = header(request, "Content-Type").unwrap_or_default();
            if !content_type
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/json")
            {
                return Err(error(
                    415,
                    "Review actions require an application/json body.",
                ));
            }
            if request
                .body_length()
                .is_some_and(|length| length as u64 > MAX_BODY_BYTES)
            {
                return Err(error(413, "Request body exceeds 64 KiB."));
            }
            let mut body = Vec::new();
            request
                .as_reader()
                .take(MAX_BODY_BYTES + 1)
                .read_to_end(&mut body)
                .map_err(|_| error(400, "Could not read the JSON request body."))?;
            if body.len() as u64 > MAX_BODY_BYTES {
                return Err(error(413, "Request body exceeds 64 KiB."));
            }
            let feedback: FeedbackRequest = serde_json::from_slice(&body)
                .map_err(|_| error(400, "Invalid review action. Use an observation ID, action, and optional correction text."))?;
            if !valid_id(&feedback.observation_id)
                || feedback
                    .value
                    .as_ref()
                    .is_some_and(|value| value.len() > 16_384)
                || (matches!(feedback.action, FeedbackAction::Correct)
                    && !feedback
                        .value
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()))
                || (!matches!(feedback.action, FeedbackAction::Correct) && feedback.value.is_some())
            {
                return Err(error(
                    400,
                    "A correction needs nonempty text of at most 16 KiB; other actions do not accept text.",
                ));
            }
            if !store
                .observations(true)
                .map_err(|_| database_error())?
                .iter()
                .any(|record| record.id == feedback.observation_id)
            {
                return Err(error(
                    404,
                    "This observation is no longer available. Refresh the workspace.",
                ));
            }
            store
                .feedback(
                    &feedback.observation_id,
                    feedback.action,
                    feedback.value.as_deref(),
                )
                .map_err(|_| database_error())?;
            Ok(json_response(200, json!({"ok":true})))
        }
        (_, "/api/feedback") => Err(error(405, "Review actions require POST with a JSON body.")),
        _ => Err(error(404, "API route not found.")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackRequest {
    observation_id: String,
    action: FeedbackAction,
    value: Option<String>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|value| value.is_ascii_hexdigit())
}

fn no_query(url: &Url) -> ApiResult<()> {
    if url.query().is_some() {
        Err(error(400, "This route does not accept query parameters."))
    } else {
        Ok(())
    }
}

fn optional_query(url: &Url, allowed: &str) -> ApiResult<Option<String>> {
    let mut value = None;
    for (key, item) in url.query_pairs() {
        if key != allowed || value.is_some() {
            return Err(error(400, "Unknown or repeated query parameter."));
        }
        value = Some(item.into_owned());
    }
    Ok(value)
}

fn database_error() -> ApiError {
    error(
        500,
        "Could not read or update the local archive. Refresh and try again.",
    )
}

fn json_response(status: u16, body: Value) -> Response<Cursor<Vec<u8>>> {
    response(
        status,
        "application/json; charset=utf-8",
        serde_json::to_vec(&body).expect("JSON values serialize"),
    )
}

fn response(status: u16, content_type: &str, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    let mut response = Response::from_data(body).with_status_code(StatusCode(status));
    for (name, value) in [
        ("Content-Type", content_type),
        ("Content-Security-Policy", CSP),
        ("Referrer-Policy", "no-referrer"),
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("Cache-Control", "no-store"),
        (
            "Permissions-Policy",
            "camera=(), microphone=(), geolocation=()",
        ),
    ] {
        response.add_header(
            Header::from_bytes(name, value).expect("static security headers are valid"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_store::{ObservationStatus, Processor};
    use crate::intelligence::extract_rules;
    use crate::model::{Conversation, Message};
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
}
