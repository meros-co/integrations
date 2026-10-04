//! `auth: oauth2` on real sockets: YouTube's spec, switched to plain HTTP,
//! against a local API and token endpoint (reached through the `token_url`
//! setting), driven through the public API.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::catalog::{Catalog, Params};
use crate::{CommandError, Connection, Core, CoreOptions, Event, OpenRequest, SettingsError};

/// How the token endpoint answers.
#[derive(Clone)]
enum Token {
    /// A new access token, with this lifetime and, when given, a rotated
    /// refresh token.
    Issue {
        access: String,
        expires_in: u64,
        rotate: Option<String>,
    },
    InvalidGrant,
}

struct Server {
    /// The access token the API accepts.
    valid: String,
    token: Token,
    /// How long the token endpoint takes to answer.
    delay: Duration,
    /// Every refresh's form body.
    refreshes: Vec<String>,
    /// Every API request's target and Authorization header.
    api: Vec<(String, String)>,
}

type Shared = Arc<Mutex<Server>>;

fn server(valid: &str, token: Token) -> Shared {
    Arc::new(Mutex::new(Server {
        valid: valid.into(),
        token,
        delay: Duration::ZERO,
        refreshes: Vec::new(),
        api: Vec::new(),
    }))
}

/// One HTTP/1.1 request: its request line, Authorization header and body.
async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<(String, String, String)> {
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
    let line = lines.next()?.to_string();
    let (mut length, mut authorization) = (0usize, String::new());
    for header in lines {
        let (name, value) = header.split_once(':')?;
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().ok()?,
            "authorization" => authorization = value.trim().to_string(),
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
        line,
        authorization,
        String::from_utf8_lossy(&body).to_string(),
    ))
}

/// The API and the token endpoint on one port.
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
                while let Some((line, authorization, body)) = read_request(&mut stream).await {
                    let target = line.split(' ').nth(1).unwrap_or("").to_string();
                    let (status, reply) = if target == "/token" {
                        let (token, delay) = {
                            let mut s = shared.lock().unwrap();
                            s.refreshes.push(body);
                            (s.token.clone(), s.delay)
                        };
                        tokio::time::sleep(delay).await;
                        match token {
                            Token::Issue {
                                access,
                                expires_in,
                                rotate,
                            } => {
                                let mut doc = json!({"access_token": access,
                                    "expires_in": expires_in, "token_type": "Bearer"});
                                if let Some(r) = rotate {
                                    doc["refresh_token"] = json!(r);
                                }
                                ("200 OK", doc)
                            }
                            Token::InvalidGrant => (
                                "400 Bad Request",
                                json!({"error": "invalid_grant",
                                       "error_description": "Token has been expired or revoked."}),
                            ),
                        }
                    } else {
                        let mut s = shared.lock().unwrap();
                        s.api.push((target, authorization.clone()));
                        if authorization == format!("Bearer {}", s.valid) {
                            (
                                "200 OK",
                                json!({"kind": "youtube#liveBroadcastListResponse", "items": []}),
                            )
                        } else {
                            ("401 Unauthorized", json!({"error": {"code": 401}}))
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

fn core() -> Core {
    let mut spec = Catalog::source_tree()
        .device("youtube-live")
        .unwrap()
        .clone();
    spec.transport.as_mut().unwrap()["scheme"] = json!("http");
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
    settings.insert("broadcast_id".into(), json!("abcDEF12345"));
    settings.insert(
        "token_url".into(),
        json!(format!("http://127.0.0.1:{port}/token")),
    );
    core.open(OpenRequest {
        device: "youtube-live".into(),
        model: "data-api-v3".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings,
        monitor: false,
    })
    .unwrap()
}

async fn get_broadcast(core: &Core, device: u64) -> crate::CommandResult {
    let params: Params = json!({"broadcast_id": "abcDEF12345"})
        .as_object()
        .unwrap()
        .clone();
    core.execute(device, "get_broadcast", params).await
}

/// Events until one matches, within 5 s.
async fn wait_event(core: &Core, matches: impl Fn(&Event) -> bool) -> Event {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            for event in core.next_events(64).await {
                if matches(&event) {
                    return event;
                }
            }
        }
    })
    .await
    .expect("event within 5 s")
}

async fn wait_connection(core: &Core, device: u64, want: fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !want(&core.snapshot(device).unwrap().connection) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connection within 5 s")
}

fn credentials(event: &Event) -> Option<&Params> {
    match event {
        Event::Credentials { settings, .. } => Some(settings),
        _ => None,
    }
}

fn refreshes(shared: &Shared) -> Vec<String> {
    shared.lock().unwrap().refreshes.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_about_to_expire_is_refreshed_first_by_a_public_client() {
    let shared = server(
        "new1",
        Token::Issue {
            access: "new1".into(),
            expires_in: 3599,
            rotate: None,
        },
    );
    let port = serve(shared.clone()).await;
    let core = core();
    // Expires in a minute, inside the 5 minute margin; no client secret.
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "refresh_token": "r1", "access_token": "old",
               "expires_at": unix() + 60}),
    );
    assert!(get_broadcast(&core, device).await.is_ok());
    assert_eq!(
        refreshes(&shared),
        ["grant_type=refresh_token&refresh_token=r1&client_id=cid"]
    );
    // The old token was never sent.
    assert!(shared
        .lock()
        .unwrap()
        .api
        .iter()
        .all(|(_, auth)| auth == "Bearer new1"));
    let event = wait_event(&core, |e| credentials(e).is_some()).await;
    let settings = credentials(&event).unwrap();
    assert_eq!(settings["access_token"], "new1");
    let expires = settings["expires_at"].as_u64().unwrap();
    assert!((unix() + 3500..=unix() + 3600).contains(&expires));
    assert!(!settings.contains_key("refresh_token"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_token_is_refreshed_and_the_request_sent_again() {
    let shared = server(
        "fresh",
        Token::Issue {
            access: "fresh".into(),
            expires_in: 3600,
            rotate: None,
        },
    );
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "client_secret": "sec", "refresh_token": "r1",
               "access_token": "revoked", "expires_at": unix() + 7200}),
    );
    assert!(get_broadcast(&core, device).await.is_ok());
    assert_eq!(
        refreshes(&shared),
        ["grant_type=refresh_token&refresh_token=r1&client_id=cid&client_secret=sec"]
    );
    let api = shared.lock().unwrap().api.clone();
    assert!(api.iter().any(|(_, a)| a == "Bearer revoked"));
    assert_eq!(api.last().unwrap().1, "Bearer fresh");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_refusal_after_a_refresh_is_terminal() {
    // The endpoint issues a token the API does not take.
    let shared = server(
        "something-else",
        Token::Issue {
            access: "fresh".into(),
            expires_in: 3600,
            rotate: None,
        },
    );
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "refresh_token": "r1", "access_token": "a",
               "expires_at": unix() + 7200}),
    );
    // The probe and the command each meet a 401: one refresh only.
    let result = get_broadcast(&core, device).await;
    assert!(
        matches!(result, Err(CommandError::Auth { .. })),
        "{result:?}"
    );
    wait_connection(&core, device, |c| {
        matches!(c, Connection::Unauthorized { .. })
    })
    .await;
    assert_eq!(refreshes(&shared).len(), 1);
    assert!(matches!(
        get_broadcast(&core, device).await,
        Err(CommandError::Auth { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_grant_is_terminal() {
    let shared = server("x", Token::InvalidGrant);
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "refresh_token": "gone"}),
    );
    let result = get_broadcast(&core, device).await;
    match result {
        Err(CommandError::Auth { message }) => {
            assert!(message.contains("invalid_grant"), "{message}");
            assert!(!message.contains("gone"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    wait_connection(&core, device, |c| {
        matches!(c, Connection::Unauthorized { .. })
    })
    .await;
    // No API request went without a token.
    assert!(shared.lock().unwrap().api.is_empty());
    assert_eq!(refreshes(&shared).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_token_endpoint_is_not_terminal() {
    // A port nobody listens on.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let shared = server("x", Token::InvalidGrant);
    let port = serve(shared.clone()).await;
    let core = core();
    let mut settings = json!({"client_id": "cid", "refresh_token": "r"})
        .as_object()
        .unwrap()
        .clone();
    settings.insert("broadcast_id".into(), json!("abcDEF12345"));
    settings.insert(
        "token_url".into(),
        json!(format!("http://127.0.0.1:{closed}/token")),
    );
    let device = core
        .open(OpenRequest {
            device: "youtube-live".into(),
            model: "data-api-v3".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings,
            monitor: false,
        })
        .unwrap();
    let result = get_broadcast(&core, device).await;
    assert!(
        matches!(&result, Err(CommandError::Transport { message }) if message.contains("could not be refreshed")),
        "{result:?}"
    );
    wait_connection(&core, device, |c| {
        matches!(c, Connection::Disconnected { .. })
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rotated_refresh_token_is_reported_and_used_next() {
    let shared = server(
        "v1",
        Token::Issue {
            access: "v1".into(),
            expires_in: 3600,
            rotate: Some("r2".into()),
        },
    );
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "refresh_token": "r1"}),
    );
    assert!(get_broadcast(&core, device).await.is_ok());
    let event = wait_event(&core, |e| credentials(e).is_some()).await;
    assert_eq!(credentials(&event).unwrap()["refresh_token"], "r2");

    // Settings changed afterwards keep the rotated token.
    let change: Params = json!({"broadcast_id": "other"})
        .as_object()
        .unwrap()
        .clone();
    core.update_settings(device, change).await.unwrap();
    // The API stops taking v1: the next refresh must use r2.
    {
        let mut s = shared.lock().unwrap();
        s.valid = "v2".into();
        s.token = Token::Issue {
            access: "v2".into(),
            expires_in: 3600,
            rotate: None,
        };
    }
    assert!(get_broadcast(&core, device).await.is_ok());
    assert_eq!(
        refreshes(&shared),
        [
            "grant_type=refresh_token&refresh_token=r1&client_id=cid",
            "grant_type=refresh_token&refresh_token=r2&client_id=cid"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_requests_share_one_refresh() {
    let shared = server(
        "t",
        Token::Issue {
            access: "t".into(),
            expires_in: 3600,
            rotate: None,
        },
    );
    shared.lock().unwrap().delay = Duration::from_millis(300);
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(
        &core,
        port,
        json!({"client_id": "cid", "refresh_token": "r"}),
    );
    let results =
        futures_util::future::join_all((0..5).map(|_| get_broadcast(&core, device))).await;
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(refreshes(&shared).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn new_settings_clear_a_refused_credential() {
    let shared = server("good", Token::InvalidGrant);
    let port = serve(shared.clone()).await;
    let core = core();
    // A plain bearer token: no refresh token, so no refresh.
    let device = open(&core, port, json!({"access_token": "bad"}));
    assert!(matches!(
        get_broadcast(&core, device).await,
        Err(CommandError::Auth { .. })
    ));
    wait_connection(&core, device, |c| {
        matches!(c, Connection::Unauthorized { .. })
    })
    .await;
    assert!(refreshes(&shared).is_empty());

    let change: Params = json!({"access_token": "good"}).as_object().unwrap().clone();
    core.update_settings(device, change).await.unwrap();
    // The probe goes again, with the new token.
    wait_connection(&core, device, |c| *c == Connection::Connected).await;
    assert!(get_broadcast(&core, device).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_or_invalid_settings_change_nothing() {
    let shared = server(
        "a",
        Token::Issue {
            access: "a".into(),
            expires_in: 3600,
            rotate: None,
        },
    );
    let port = serve(shared.clone()).await;
    let core = core();
    let device = open(&core, port, json!({"access_token": "a"}));
    let unknown: Params = json!({"no_such_setting": 1}).as_object().unwrap().clone();
    assert!(matches!(
        core.update_settings(device, unknown).await,
        Err(SettingsError::InvalidSettings { message }) if message.contains("no_such_setting")
    ));
    let wrong: Params = json!({"expires_at": "soon"}).as_object().unwrap().clone();
    assert!(matches!(
        core.update_settings(device, wrong).await,
        Err(SettingsError::InvalidSettings { .. })
    ));
    let remote: Params = json!({"token_url": "http://192.0.2.1/token"})
        .as_object()
        .unwrap()
        .clone();
    assert!(matches!(
        core.update_settings(device, remote).await,
        Err(SettingsError::InvalidSettings { .. })
    ));
    // Still working with the old settings.
    assert!(get_broadcast(&core, device).await.is_ok());
    assert_eq!(
        core.update_settings(device + 100, Params::new()).await,
        Err(SettingsError::Closed)
    );
}
