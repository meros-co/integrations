//! The integrations core as a local HTTP service.
//!
//! For hosts that cannot link the core, such as PHP. Every response body is
//! produced by `meros_integrations::json`, the same functions every other
//! delivery uses, so a consumer of this service sees exactly what a linked
//! consumer sees.
//!
//! ```text
//! meros-integrations serve [--listen 127.0.0.1:47800] [--token-file PATH]
//! ```
//!
//! | Method and path              | Body / query                          | Returns                    |
//! |------------------------------|---------------------------------------|----------------------------|
//! | GET  /v1/catalog             |                                       | the catalogue              |
//! | POST /v1/open                | an OpenRequest                        | {device} or {error}        |
//! | POST /v1/execute             | {device, command, params}             | {ok} or {error}            |
//! | GET  /v1/snapshot/{device}   |                                       | snapshot, or null          |
//! | POST /v1/close               | {device}                              | {}                         |
//! | GET  /v1/events              | ?max=256&wait_ms=25000                | [events], empty on timeout |
//!
//! Every request needs `Authorization: Bearer <token>`. The token is read from
//! the token file, which is created with a random token if absent. The service
//! listens on loopback unless told otherwise.
//!
//! Events are one queue: run one event consumer per service.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use meros_integrations::{json as api, Core};
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_LISTEN: &str = "127.0.0.1:47800";
/// Long-poll ceiling, under common HTTP client timeouts.
const MAX_WAIT: Duration = Duration::from_secs(25);

struct App {
    core: Core,
    token: String,
}

type Shared = Arc<App>;

fn authorized(app: &App, headers: &HeaderMap) -> bool {
    let given = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    // Constant-time comparison: the token guards control of show equipment.
    given.is_some_and(|g| {
        g.len() == app.token.len()
            && g.bytes()
                .zip(app.token.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    })
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"error": "unauthorized"}})),
    )
        .into_response()
}

async fn catalog(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::catalog(&app.core)).into_response()
}

async fn open(State(app): State<Shared>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::open(&app.core, &body)).into_response()
}

#[derive(Deserialize)]
struct Execute {
    device: u64,
    command: String,
    #[serde(default)]
    params: Value,
}

async fn execute(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Execute>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::execute(&app.core, body.device, &body.command, &body.params).await).into_response()
}

async fn snapshot(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(device): Path<u64>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::snapshot(&app.core, device)).into_response()
}

#[derive(Deserialize)]
struct Close {
    device: u64,
}

async fn close(State(app): State<Shared>, headers: HeaderMap, Json(body): Json<Close>) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    app.core.close(body.device).await;
    Json(json!({})).into_response()
}

#[derive(Deserialize)]
struct EventsQuery {
    max: Option<usize>,
    wait_ms: Option<u64>,
}

async fn events(
    State(app): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    let max = q.max.unwrap_or(256).clamp(1, 10_000);
    let wait = Duration::from_millis(q.wait_ms.unwrap_or(25_000)).min(MAX_WAIT);
    let events = tokio::time::timeout(wait, app.core.next_events(max))
        .await
        .unwrap_or_default();
    Json(api::events(&events)).into_response()
}

fn router(app: Shared) -> Router {
    Router::new()
        .route("/v1/catalog", get(catalog))
        .route("/v1/open", post(open))
        .route("/v1/execute", post(execute))
        .route("/v1/snapshot/{device}", get(snapshot))
        .route("/v1/close", post(close))
        .route("/v1/events", get(events))
        .with_state(app)
}

fn load_or_create_token(path: &PathBuf) -> std::io::Result<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let token = existing.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, &token)?;
    Ok(token)
}

struct Args {
    listen: SocketAddr,
    token_file: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => {}
        _ => {
            return Err(
                "usage: meros-integrations serve [--listen ADDR:PORT] [--token-file PATH]".into(),
            )
        }
    }
    let mut listen: SocketAddr = DEFAULT_LISTEN.parse().unwrap();
    let mut token_file = PathBuf::from("meros-integrations.token");
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--listen" => listen = value.parse().map_err(|e| format!("--listen: {e}"))?,
            "--token-file" => token_file = PathBuf::from(value),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args { listen, token_file })
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let token = load_or_create_token(&args.token_file).unwrap_or_else(|e| {
        eprintln!("cannot read or create {}: {e}", args.token_file.display());
        std::process::exit(1);
    });
    let core = Core::new().expect("start core");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(args.listen)
            .await
            .unwrap_or_else(|e| {
                eprintln!("cannot listen on {}: {e}", args.listen);
                std::process::exit(1);
            });
        // One line for supervisors and tests: where it is listening.
        println!(
            "{}",
            json!({"listening": listener.local_addr().unwrap().to_string()})
        );
        let app = Arc::new(App { core, token });
        axum::serve(listener, router(app))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .expect("serve");
    });
}
