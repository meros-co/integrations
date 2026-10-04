//! `oauth.validate` on real sockets: Twitch's spec, switched to plain HTTP,
//! against a local Helix API, token endpoint and validation endpoint on one
//! port, driven through the public API.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::catalog::Catalog;
use crate::{Connection, Core, CoreOptions, Event, OpenRequest};

struct Server {
    /// The access tokens the validation endpoint accepts.
    valid: BTreeSet<String>,
    /// The access token the token endpoint issues.
    issue: String,
    /// The validation endpoint answers this status this many times first.
    fail: Option<(u16, usize)>,
    /// Every validation's Authorization header, and whether it carried a
    /// Client-Id.
    validations: Vec<(String, bool)>,
    /// Every refresh's form body.
    refreshes: Vec<String>,
}

type Shared = Arc<Mutex<Server>>;

fn server(valid: &[&str], issue: &str) -> Shared {
    Arc::new(Mutex::new(Server {
        valid: valid.iter().map(|s| s.to_string()).collect(),
        issue: issue.into(),
        fail: None,
        validations: Vec::new(),
        refreshes: Vec::new(),
    }))
}

/// One request: its target, Authorization header, whether it had a
/// Client-Id, and its body.
async fn read_request(
    stream: &mut tokio::net::TcpStream,
) -> Option<(String, String, bool, String)> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let target = lines.next()?.split(' ').nth(1)?.to_string();
    let (mut length, mut authorization, mut client_id) = (0usize, String::new(), false);
    for header in lines {
        let (name, value) = header.split_once(':')?;
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().ok()?,
            "authorization" => authorization = value.trim().to_string(),
            "client-id" => client_id = true,
            _ => {}
        }
    }
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some((
        target,
        authorization,
        client_id,
        String::from_utf8_lossy(&body).to_string(),
    ))
}

async fn serve(shared: Shared) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let shared = shared.clone();
            tokio::spawn(async move {
                while let Some((target, authorization, client_id, body)) =
                    read_request(&mut stream).await
                {
                    let (status, reply) = {
                        let mut s = shared.lock().unwrap();
                        if target == "/oauth2/token" {
                            s.refreshes.push(body);
                            (
                                "200 OK",
                                json!({"access_token": s.issue, "expires_in": 14000,
                                       "token_type": "bearer"}),
                            )
                        } else if target == "/oauth2/validate" {
                            s.validations.push((authorization.clone(), client_id));
                            let token = authorization.strip_prefix("OAuth ").unwrap_or("");
                            let valid = s.valid.contains(token);
                            match s.fail.as_mut() {
                                Some((_, left)) if *left > 0 => {
                                    *left -= 1;
                                    ("503 Service Unavailable", json!({"status": 503}))
                                }
                                _ if valid => (
                                    "200 OK",
                                    json!({"client_id": "cid", "login": "validatedlogin",
                                           "scopes": [], "user_id": "141981764",
                                           "expires_in": 5520}),
                                ),
                                _ => (
                                    "401 Unauthorized",
                                    json!({"status": 401, "message": "invalid access token"}),
                                ),
                            }
                        } else {
                            ("200 OK", json!({"data": []}))
                        }
                    };
                    let body = reply.to_string();
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if stream.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// Twitch over plain HTTP, its token and validation endpoints local, and
/// validated every `every_s`.
fn core(port: u16, every_s: u64) -> Core {
    let mut spec = Catalog::source_tree().device("twitch").unwrap().clone();
    let transport = spec.transport.as_mut().unwrap();
    transport["scheme"] = json!("http");
    transport["oauth"]["validate"]["url"] =
        json!(format!("http://127.0.0.1:{port}/oauth2/validate"));
    transport["oauth"]["validate"]["every_s"] = json!(every_s);
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    Core::with_catalog(catalog, CoreOptions::new()).unwrap()
}

fn unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn open(core: &Core, port: u16, settings: Value) -> u64 {
    let mut settings = settings.as_object().unwrap().clone();
    settings.insert("client_id".into(), json!("cid"));
    settings.insert("broadcaster_id".into(), json!("141981764"));
    settings.insert(
        "token_url".into(),
        json!(format!("http://127.0.0.1:{port}/oauth2/token")),
    );
    core.open(OpenRequest {
        device: "twitch".into(),
        model: "helix".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings,
        // Validation is Twitch's requirement, not telemetry.
        monitor: false,
    })
    .unwrap()
}

fn validations(shared: &Shared) -> Vec<String> {
    let s = shared.lock().unwrap();
    assert!(
        s.validations.iter().all(|(_, client_id)| !client_id),
        "a validation carried the device's Client-Id"
    );
    s.validations.iter().map(|(a, _)| a.clone()).collect()
}

/// Wait up to `secs` for a condition.
async fn wait_until(secs: u64, mut done: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(secs), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("condition within the time allowed");
}

fn unauthorized(core: &Core, device: u64) -> bool {
    matches!(
        core.snapshot(device).unwrap().connection,
        Connection::Unauthorized { .. }
    )
}

/// Every event so far, without waiting.
async fn drain(core: &Core) -> Vec<Event> {
    let mut all = Vec::new();
    loop {
        let batch = tokio::time::timeout(Duration::from_millis(100), core.next_events(256)).await;
        match batch {
            Ok(events) if !events.is_empty() => all.extend(events),
            _ => return all,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_token_is_validated_on_open_and_every_interval_unmonitored() {
    let shared = server(&["s3cr3t"], "unused");
    let port = serve(shared.clone()).await;
    let core = core(port, 1);
    let device = open(
        &core,
        port,
        json!({"access_token": "s3cr3t", "expires_at": unix() + 7200}),
    );
    wait_until(5, || validations(&shared).len() >= 3).await;
    assert!(validations(&shared).iter().all(|a| a == "OAuth s3cr3t"));
    assert!(shared.lock().unwrap().refreshes.is_empty());
    assert!(!unauthorized(&core, device));
    // Nothing of the answer reaches state or the logs.
    let state = core.snapshot(device).unwrap().state.to_string();
    assert!(!state.contains("validatedlogin"), "{state}");
    for event in drain(&core).await {
        if let Event::Log { message, .. } = event {
            assert!(
                !message.contains("s3cr3t") && !message.contains("validatedlogin"),
                "{message}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_access_token_the_refreshed_one_is_validated() {
    let shared = server(&["fresh"], "fresh");
    let port = serve(shared.clone()).await;
    let core = core(port, 3600);
    let device = open(&core, port, json!({"refresh_token": "r1"}));
    wait_until(5, || !validations(&shared).is_empty()).await;
    assert_eq!(validations(&shared), ["OAuth fresh"]);
    assert_eq!(shared.lock().unwrap().refreshes.len(), 1);
    assert!(!unauthorized(&core, device));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_validation_refreshes_once_and_validates_the_new_token() {
    let shared = server(&["fresh"], "fresh");
    let port = serve(shared.clone()).await;
    let core = core(port, 3600);
    let device = open(
        &core,
        port,
        json!({"refresh_token": "r1", "access_token": "revoked", "expires_at": unix() + 7200}),
    );
    wait_until(5, || validations(&shared).len() >= 2).await;
    assert_eq!(validations(&shared), ["OAuth revoked", "OAuth fresh"]);
    assert_eq!(shared.lock().unwrap().refreshes.len(), 1);
    let events = drain(&core).await;
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Credentials { settings, .. }
        if settings["access_token"] == "fresh")));
    assert!(!unauthorized(&core, device));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_validation_of_the_refreshed_token_is_terminal() {
    // The token endpoint issues a token validation refuses too.
    let shared = server(&[], "also-revoked");
    let port = serve(shared.clone()).await;
    let core = core(port, 3600);
    let device = open(
        &core,
        port,
        json!({"refresh_token": "r1", "access_token": "revoked", "expires_at": unix() + 7200}),
    );
    wait_until(5, || unauthorized(&core, device)).await;
    assert_eq!(
        validations(&shared),
        ["OAuth revoked", "OAuth also-revoked"]
    );
    assert_eq!(shared.lock().unwrap().refreshes.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_refresh_token_a_refused_validation_is_terminal() {
    let shared = server(&[], "unused");
    let port = serve(shared.clone()).await;
    let core = core(port, 3600);
    let device = open(&core, port, json!({"access_token": "revoked"}));
    wait_until(5, || unauthorized(&core, device)).await;
    assert_eq!(validations(&shared), ["OAuth revoked"]);
    assert!(shared.lock().unwrap().refreshes.is_empty());
    // A new token from a new sign-in lets it go again, and is validated.
    shared
        .lock()
        .unwrap()
        .valid
        .insert("signed-in-again".into());
    let change = json!({"access_token": "signed-in-again"})
        .as_object()
        .unwrap()
        .clone();
    core.update_settings(device, change).await.unwrap();
    wait_until(5, || validations(&shared).len() >= 2).await;
    assert_eq!(validations(&shared)[1], "OAuth signed-in-again");
    wait_until(5, || {
        core.snapshot(device).unwrap().connection == Connection::Connected
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_validation_endpoint_is_retried_not_terminal() {
    let shared = server(&["s3cr3t"], "unused");
    shared.lock().unwrap().fail = Some((503, 2));
    let port = serve(shared.clone()).await;
    let core = core(port, 3600);
    let device = open(&core, port, json!({"access_token": "s3cr3t"}));
    // 503, then 1 s, 503, then 2 s, 200.
    wait_until(8, || validations(&shared).len() >= 3).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(validations(&shared).len(), 3);
    assert!(!unauthorized(&core, device));
    let events = drain(&core).await;
    assert!(events.iter().any(|e| matches!(e, Event::Log { message, .. }
        if message.contains("could not be validated: HTTP 503"))));
}
