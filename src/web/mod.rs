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

mod api;

use api::api;
use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use anyhow::{Context, Result};
use reqwest::Url;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::store::Store;

const MAX_BODY_BYTES: u64 = 64 * 1024;
const MAX_URL_BYTES: usize = 8192;
const HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/index.html"));
const SCRIPT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.js"));
const STYLE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/style.css"));
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
mod tests;
