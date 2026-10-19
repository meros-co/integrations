//! The integrations core as a local HTTP service.
//!
//! For hosts that cannot link the core, such as PHP. Every response body is
//! produced by `meros_integrations::json`, the same functions every other
//! delivery uses, so a consumer of this service sees exactly what a linked
//! consumer sees.
//!
//! ```text
//! meros-integrations serve [--listen 127.0.0.1:47800] [--token-file PATH]
//!                          [--bind-address ADDR] [--devices ID,ID,...]
//! ```
//!
//! `--devices` names the only integrations the service works with (spec ids,
//! vendor groups such as `vendor-sennheiser`, or `all`), for a product that
//! uses only some: the catalogue lists only them, opening any other is refused
//! with `not_selected`, and discovery runs only the protocols that find them.
//! A name the build does not include stops the service at startup. Which
//! integrations are built into the service at all is chosen when it is built:
//! `cargo build -p meros-integrations-sidecar --no-default-features
//! --features meros-integrations/sennheiser-ew-dx,...`.
//!
//! | Method and path              | Body / query                          | Returns                    |
//! |------------------------------|---------------------------------------|----------------------------|
//! | GET  /v1/catalog             |                                       | the catalogue              |
//! | POST /v1/open                | an OpenRequest                        | {device} or {error}        |
//! | POST /v1/discover            | {action, protocols?, hints?}          | {ok} or {error}            |
//! | POST /v1/execute             | {device, command, params}             | {ok} or {error}            |
//! | POST /v1/settings            | {device, settings}                    | {ok: true} or {error}      |
//! | GET  /v1/snapshot/{device}   |                                       | snapshot, or null          |
//! | POST /v1/close               | {device}                              | {}                         |
//! | POST /v1/reconnect           | {device}                              | {reconnected: true/false}  |
//! | POST /v1/reconnect_all       |                                       | {}                         |
//! | GET  /v1/events              | ?max=256&wait_ms=25000                | [events], empty on timeout |
//! | GET  /v1/devices/{device}/streams/{stream}.mjpg |                    | MJPEG, until closed        |
//! | GET  /v1/devices/{device}/streams/{stream}.jpg  | ?wait_ms=5000      | the newest JPEG            |
//! | GET  /v1/devices/{device}/streams/{stream}.frames |                  | preview frames, until closed |
//!
//! `/v1/discover` takes `protocols` among `mcp` (Sennheiser G3/G4), `ssdp`
//! (Sony cameras), `pjlink` (PJLink projectors) and `mdns` (Blackmagic ATEM,
//! Videohub, HyperDeck, SmartView and MultiView, and Meros Cameras); none
//! means every protocol that finds a device in the service's catalogue.
//! `hints` are addresses where devices were last seen.
//!
//! `/v1/settings` changes an open device's settings without closing it, as
//! `update_settings` does in every delivery: `settings` are merged into the
//! device's current ones and validated against its spec first (an unknown
//! setting is `invalid_settings` and changes nothing). A spec-driven device
//! takes them live, and one whose credential was refused connects again; a
//! native one is restarted under the same id. After a sign-in the consumer
//! passes the new OAuth tokens here.
//!
//! `/v1/reconnect` drops a device's connections and opens them again now,
//! from its current settings, rather than after a retry delay, as
//! `reconnect` does in every delivery: for a host that resumes after being
//! suspended or whose network changed. The device keeps its id and last
//! known state, which is read afresh; commands in flight fail with
//! `transport`. It answers once the device has restarted (not once it is
//! connected), with `reconnected: false` if the device is not open.
//! `/v1/reconnect_all` does the same for every open device.
//!
//! An `auth: oauth2` device (YouTube, Planning Center) refreshes its access
//! token itself and reports each new token in a `credentials` event (SPEC.md,
//! Events). The service keeps tokens in memory only: the consumer persists
//! them from the events and passes them when it opens the device again.
//!
//! Every request needs `Authorization: Bearer <token>`. The token is read from
//! the token file, which is created with a random token if absent. The service
//! listens on loopback unless told otherwise.
//!
//! Streams (a camera's live view, declared under a device's `streams` in the
//! catalogue) are served as media rather than JSON, by their format. A `jpeg`
//! stream is served as `.mjpg`, `multipart/x-mixed-replace`, which a browser's
//! `<img>` shows as live video, and as `.jpg`, one JPEG. Since an `<img>`
//! cannot send a header, these two also accept the token as
//! `?access_token=<token>` (RFC 6750 §2.3); a URL holding the token can end up
//! in logs and history, so prefer the header where the client can send one.
//! The device produces frames only while a stream is being read, and a slow
//! reader gets the newest frame, never a backlog.
//!
//! A `meros-preview` stream (a Meros Camera's `live`) is served as `.frames`,
//! `Content-Type: application/x-meros-preview`, chunked until the client
//! closes: the frames back to back exactly as the camera frames them (SPEC.md,
//! Streams), each a u32 length and an i64 presentation time in microseconds,
//! both big-endian, a flags byte whose bit 0 marks a keyframe, then the
//! H.264 or HEVC access unit in Annex-B. The first frame is a keyframe, and
//! after frames a slow reader missed the next frame sent is again a
//! keyframe, so a decoder never gets a frame whose references it lacks; the
//! presentation times show the gap. `.frames` needs the header: it does not
//! take the token in the query.
//!
//! A refused stream is answered with a JSON error and status 404 (unknown
//! stream, or device not open) or 406 (a stream whose format that extension
//! does not carry, such as `.frames` for a `jpeg` stream or `.mjpg` for a
//! `meros-preview` one).
//!
//! Events are one queue: run one event consumer per service. Their kinds and
//! shapes are those of every delivery (SPEC.md, Events), including `message`
//! events from a listener such as `osc-listener`, which a POST /v1/open opens
//! with no `host`.
//!
//! SIGTERM (systemd's stop) and SIGINT stop the service cleanly, as do
//! Ctrl-C, closing the console and a system shutdown on Windows: it stops
//! taking requests, lets those in flight finish (up to 10 s), refuses new
//! devices and commands, gives commands already sent up to 5 s, then closes
//! every device so each ends what it started on the device (subscriptions,
//! metering, sessions) before the process exits.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, StatusCode};
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
    token_matches(app, given)
}

fn token_matches(app: &App, given: Option<&str>) -> bool {
    // Constant-time comparison: the token guards control of show equipment.
    given.is_some_and(|g| {
        g.len() == app.token.len()
            && g.bytes()
                .zip(app.token.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    })
}

/// The header, or for an image a client cannot add headers to, the
/// `access_token` query parameter (RFC 6750 §2.3).
fn authorized_or_query(app: &App, headers: &HeaderMap, token: Option<&str>) -> bool {
    authorized(app, headers) || token_matches(app, token)
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

async fn discover(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::discover(&app.core, &body)).into_response()
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
struct Settings {
    device: u64,
    #[serde(default)]
    settings: Value,
}

async fn settings(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Settings>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(api::update_settings(&app.core, body.device, &body.settings).await).into_response()
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

async fn reconnect(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Close>,
) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    Json(json!({"reconnected": app.core.reconnect(body.device).await})).into_response()
}

async fn reconnect_all(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers) {
        return unauthorized();
    }
    app.core.reconnect_all().await;
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

#[derive(Deserialize)]
struct StreamQuery {
    access_token: Option<String>,
    wait_ms: Option<u64>,
}

/// How long a single-frame request waits for the device's first frame.
const DEFAULT_FRAME_WAIT: Duration = Duration::from_secs(5);
const MJPEG_BOUNDARY: &str = "meros-frame";

fn stream_refused(error: Value) -> Response {
    let status = match error["error"]["error"].as_str() {
        Some("not_acceptable") => StatusCode::NOT_ACCEPTABLE,
        _ => StatusCode::NOT_FOUND,
    };
    (status, Json(error)).into_response()
}

/// One part of a `multipart/x-mixed-replace` body (RFC 2046 §5.1.1).
fn mjpeg_part(frame: &meros_integrations::Frame) -> Vec<u8> {
    let mut part = format!(
        "--{MJPEG_BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nX-Sequence: {}\r\nX-Dropped: {}\r\n\r\n",
        frame.data.len(),
        frame.sequence,
        frame.dropped
    )
    .into_bytes();
    part.extend_from_slice(&frame.data);
    part.extend_from_slice(b"\r\n");
    part
}

/// What a stream URL asks for, by its extension.
#[derive(Clone, Copy, PartialEq)]
enum StreamFile {
    /// `.mjpg`: JPEG frames as `multipart/x-mixed-replace`.
    Mjpeg,
    /// `.jpg`: one JPEG.
    Jpeg,
    /// `.frames`: `meros-preview` frames back to back.
    Frames,
}

impl StreamFile {
    fn parse(file: &str) -> Option<(&str, Self)> {
        [
            (".mjpg", Self::Mjpeg),
            (".jpg", Self::Jpeg),
            (".frames", Self::Frames),
        ]
        .into_iter()
        .find_map(|(suffix, kind)| file.strip_suffix(suffix).map(|name| (name, kind)))
    }

    /// The one stream format this URL carries.
    fn format(self) -> &'static str {
        match self {
            Self::Mjpeg | Self::Jpeg => "jpeg",
            Self::Frames => PREVIEW_FORMAT,
        }
    }
}

const PREVIEW_FORMAT: &str = "meros-preview";
const PREVIEW_CONTENT_TYPE: &str = "application/x-meros-preview";
/// A `meros-preview` frame's header: u32 length, i64 pts, u8 flags.
const PREVIEW_HEADER: usize = 13;

/// Passes a `meros-preview` stream's frames from its first keyframe on and,
/// after any frame this reader missed (one replaced in its slot, or a gap in
/// the sequence), none until the next keyframe: a decoder never gets a frame
/// whose reference frames it did not get.
#[derive(Default)]
struct KeyframeGate {
    next_sequence: Option<u64>,
    in_sync: bool,
}

impl KeyframeGate {
    fn pass(&mut self, frame: &meros_integrations::Frame) -> bool {
        let contiguous =
            frame.dropped == 0 && self.next_sequence.is_none_or(|n| frame.sequence == n);
        self.next_sequence = Some(frame.sequence + 1);
        if !contiguous {
            self.in_sync = false;
        }
        if frame.data.len() >= PREVIEW_HEADER && frame.data[PREVIEW_HEADER - 1] & 1 == 1 {
            self.in_sync = true;
        }
        self.in_sync
    }
}

async fn device_stream(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path((device, file)): Path<(u64, String)>,
    Query(q): Query<StreamQuery>,
) -> Response {
    let parsed = StreamFile::parse(&file);
    // `.frames` is read by a player that can send the header, never by an
    // `<img>`, so it does not take the token in the query.
    let allowed = match parsed {
        Some((_, StreamFile::Frames)) => authorized(&app, &headers),
        _ => authorized_or_query(&app, &headers, q.access_token.as_deref()),
    };
    if !allowed {
        return unauthorized();
    }
    let Some((name, kind)) = parsed else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": {"error": "invalid_request",
                "message": "ask for <stream>.mjpg, <stream>.jpg or <stream>.frames"}})),
        )
            .into_response();
    };
    let handle = match api::open_stream(&app.core, device, name) {
        Ok(handle) => handle,
        Err(error) => return stream_refused(error),
    };
    if handle.format() != kind.format() {
        let instead = match handle.format() {
            "jpeg" => format!("; ask for {name}.mjpg or {name}.jpg"),
            PREVIEW_FORMAT => format!("; ask for {name}.frames"),
            _ => String::new(),
        };
        return stream_refused(json!({"error": {"error": "not_acceptable",
            "message": format!("stream '{name}' is {}, not {}{instead}",
                handle.format(), kind.format())}}));
    }
    if kind == StreamFile::Frames {
        let frames = futures_util::stream::unfold(
            (handle, KeyframeGate::default()),
            |(handle, mut gate)| async move {
                loop {
                    let frame = handle.next_frame().await?;
                    if gate.pass(&frame) {
                        let bytes = frame.data.to_vec();
                        return Some((Ok::<_, std::convert::Infallible>(bytes), (handle, gate)));
                    }
                }
            },
        );
        return (
            [
                (header::CONTENT_TYPE, PREVIEW_CONTENT_TYPE.to_string()),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            Body::from_stream(frames),
        )
            .into_response();
    }
    if kind == StreamFile::Mjpeg {
        use futures_util::StreamExt;
        let frames = futures_util::stream::unfold(handle, |handle| async move {
            let frame = handle.next_frame().await?;
            Some((
                Ok::<_, std::convert::Infallible>(mjpeg_part(&frame)),
                handle,
            ))
        })
        .chain(futures_util::stream::once(async {
            Ok(format!("--{MJPEG_BOUNDARY}--\r\n").into_bytes())
        }));
        return (
            [
                (
                    header::CONTENT_TYPE,
                    format!("multipart/x-mixed-replace; boundary={MJPEG_BOUNDARY}"),
                ),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            Body::from_stream(frames),
        )
            .into_response();
    }
    let wait = q
        .wait_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_FRAME_WAIT)
        .min(MAX_WAIT);
    match tokio::time::timeout(wait, handle.next_frame()).await {
        Ok(Some(frame)) => (
            [
                (header::CONTENT_TYPE, "image/jpeg".to_string()),
                (header::CACHE_CONTROL, "no-store".to_string()),
                (
                    HeaderName::from_static("x-sequence"),
                    frame.sequence.to_string(),
                ),
                (
                    HeaderName::from_static("x-dropped"),
                    frame.dropped.to_string(),
                ),
            ],
            frame.data.to_vec(),
        )
            .into_response(),
        Ok(None) => {
            stream_refused(json!({"error": {"error": "closed", "message": "session closed"}}))
        }
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({"error": {"error": "timeout",
                "message": "the device sent no frame in time"}})),
        )
            .into_response(),
    }
}

fn router(app: Shared) -> Router {
    Router::new()
        .route("/v1/catalog", get(catalog))
        .route("/v1/open", post(open))
        .route("/v1/discover", post(discover))
        .route("/v1/execute", post(execute))
        .route("/v1/settings", post(settings))
        .route("/v1/snapshot/{device}", get(snapshot))
        .route("/v1/close", post(close))
        .route("/v1/reconnect", post(reconnect))
        .route("/v1/reconnect_all", post(reconnect_all))
        .route("/v1/events", get(events))
        .route("/v1/devices/{device}/streams/{file}", get(device_stream))
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
    options: meros_integrations::CoreOptions,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => {}
        _ => {
            return Err(
                "usage: meros-integrations serve [--listen ADDR:PORT] [--token-file PATH] [--bind-address ADDR] [--devices ID,ID,...]".into(),
            )
        }
    }
    let mut listen: SocketAddr = DEFAULT_LISTEN.parse().unwrap();
    let mut token_file = PathBuf::from("meros-integrations.token");
    let mut options = meros_integrations::CoreOptions::new();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--listen" => listen = value.parse().map_err(|e| format!("--listen: {e}"))?,
            "--token-file" => token_file = PathBuf::from(value),
            "--bind-address" => {
                options =
                    options.bind_address(value.parse().map_err(|e| format!("--bind-address: {e}"))?)
            }
            // The only integrations this sidecar works with: spec ids, vendor
            // groups or `all`; every integration in the build when absent.
            "--devices" => {
                options =
                    options.devices(value.split(',').map(str::trim).filter(|id| !id.is_empty()))
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        listen,
        token_file,
        options,
    })
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
    let core = Core::with_options(args.options).unwrap_or_else(|e| {
        eprintln!("cannot start the core: {e}");
        std::process::exit(2);
    });
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
        let (stop, stopping) = tokio::sync::watch::channel(false);
        let signalled = app.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            // Long polls answer now, so the requests in flight can finish.
            signalled.core.interrupt_events();
            let _ = stop.send(true);
        });
        let mut until_stop = stopping.clone();
        let server =
            axum::serve(listener, router(app.clone())).with_graceful_shutdown(async move {
                let _ = until_stop.wait_for(|s| *s).await;
            });
        let mut after_stop = stopping;
        tokio::select! {
            result = server => result.expect("serve"),
            // Requests still open this long after the signal (an MJPEG
            // stream, a command to a device that never answers) are cut off.
            _ = async move {
                let _ = after_stop.wait_for(|s| *s).await;
                tokio::time::sleep(REQUEST_GRACE).await;
            } => {}
        }
        // Then every device is closed cleanly: subscriptions and metering
        // ended, last messages written.
        app.core.close_all(CLOSE_GRACE).await;
        eprintln!("stopped");
    });
}

/// How long requests in flight get to finish after a stop signal.
const REQUEST_GRACE: Duration = Duration::from_secs(10);
/// How long commands still in flight get before devices are closed.
const CLOSE_GRACE: Duration = Duration::from_secs(5);

/// SIGINT or SIGTERM (systemd's stop) on Unix; Ctrl-C, closing the console
/// window or a system shutdown on Windows.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_close, ctrl_shutdown};
        let mut close = ctrl_close().expect("console close handler");
        let mut shutdown = ctrl_shutdown().expect("shutdown handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = close.recv() => {}
            _ = shutdown.recv() => {}
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(sequence: u64, dropped: u64, keyframe: bool) -> meros_integrations::Frame {
        let mut data = 2u32.to_be_bytes().to_vec();
        data.extend_from_slice(&(sequence as i64 * 40_000).to_be_bytes());
        data.push(keyframe as u8);
        data.extend_from_slice(&[0x65, 0]);
        meros_integrations::Frame {
            format: PREVIEW_FORMAT,
            data: data.into(),
            sequence,
            dropped,
        }
    }

    #[test]
    fn the_preview_starts_on_a_keyframe_and_resyncs_after_a_gap() {
        let mut gate = KeyframeGate::default();
        let passed: Vec<bool> = [
            frame(7, 0, false),  // joined mid-GOP: held back
            frame(8, 0, true),   // the first keyframe
            frame(9, 0, false),  // its GOP
            frame(11, 1, false), // one replaced in the slot
            frame(12, 0, false), // still waiting
            frame(13, 0, true),  // in sync again
            frame(14, 0, false),
            frame(16, 0, false), // a gap in the sequence alone
            frame(17, 0, true),
        ]
        .iter()
        .map(|f| gate.pass(f))
        .collect();
        assert_eq!(
            passed,
            [false, true, true, false, false, true, true, false, true]
        );
    }

    #[test]
    fn stream_urls_name_their_format() {
        assert!(matches!(
            StreamFile::parse("live.frames"),
            Some(("live", StreamFile::Frames))
        ));
        assert!(matches!(
            StreamFile::parse("live.mjpg"),
            Some(("live", StreamFile::Mjpeg))
        ));
        assert!(matches!(
            StreamFile::parse("live.jpg"),
            Some(("live", StreamFile::Jpeg))
        ));
        assert!(StreamFile::parse("live.h264").is_none());
        assert_eq!(StreamFile::Frames.format(), "meros-preview");
    }
}
