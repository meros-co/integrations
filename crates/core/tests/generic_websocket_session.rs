//! The generic WebSocket module against real local WebSocket servers, driven
//! through the public API.
//!
//! The simulator (RFC 6455) checks the opening handshake's credentials and
//! subprotocol offer, greets each connection with a JSON message, echoes text
//! and binary messages, answers JSON carrying an `id` with that id, answers
//! pings (the library does), and closes with 4000 when sent `close`. Others
//! serve wss with a self-signed certificate, never read (so never answer a
//! ping), or refuse the handshake with 401.

#![cfg(feature = "generic-websocket")]
// tungstenite's handshake callback signature returns a large error type.
#![allow(clippy::result_large_err)]

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{CommandError, Connection, Core, DeviceId, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, settings: Value) -> DeviceId {
    core.open(OpenRequest {
        device: "generic-websocket".into(),
        model: "generic".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(settings),
        monitor: true,
    })
    .unwrap()
}

async fn wait_for_state(core: &Core, id: DeviceId, pred: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if pred(&state) {
                return state;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

async fn wait_for_connection(core: &Core, id: DeviceId, want: impl Fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if want(&core.snapshot(id).unwrap().connection) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("connection state within 10 s")
}

#[derive(Default)]
struct Seen {
    connections: AtomicUsize,
    authorization: Mutex<Vec<String>>,
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(stream: S, seen: Arc<Seen>) {
    let callback = |request: &Request, mut response: Response| {
        let header = |name: &str| {
            request
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string()
        };
        seen.authorization
            .lock()
            .unwrap()
            .push(header("Authorization"));
        // Choose the first offer this server speaks.
        let offered = header("Sec-WebSocket-Protocol");
        if offered.split(',').any(|p| p.trim() == "v1.json") {
            response
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", "v1.json".parse().unwrap());
        }
        Ok(response)
    };
    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await else {
        return;
    };
    seen.connections.fetch_add(1, Ordering::SeqCst);
    let hello = json!({"type": "hello", "n": seen.connections.load(Ordering::SeqCst)});
    if ws.send(Message::text(hello.to_string())).await.is_err() {
        return;
    }
    while let Some(Ok(message)) = ws.next().await {
        let reply = match message {
            Message::Text(text) if text.as_str() == "close" => {
                let _ = ws
                    .close(Some(CloseFrame {
                        code: CloseCode::from(4000),
                        reason: "bye".into(),
                    }))
                    .await;
                return;
            }
            Message::Text(text) => match serde_json::from_str::<Value>(&text) {
                Ok(v) if v.get("id").is_some() => {
                    // Something unasked first, which is not the reply.
                    let _ = ws
                        .send(Message::text(json!({"type": "noise"}).to_string()))
                        .await;
                    Message::text(json!({"id": v["id"], "ok": true}).to_string())
                }
                _ => Message::Text(text),
            },
            Message::Binary(data) => Message::Binary(data),
            _ => continue,
        };
        if ws.send(reply).await.is_err() {
            return;
        }
    }
}

async fn server() -> (u16, Arc<Seen>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Seen::default());
    let shared = seen.clone();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            tokio::spawn(serve(tcp, shared.clone()));
        }
    });
    (port, seen)
}

#[tokio::test(flavor = "multi_thread")]
async fn text_binary_json_requests_pings_close_and_reconnect() {
    let (port, seen) = server().await;
    let core = common::core().unwrap();
    let id = open(
        &core,
        port,
        json!({
            "path": "/api?v=1", "subprotocols": "v2.json, v1.json",
            "auth": "bearer", "token": "tok",
            "key_path": "$.type", "ping_interval_ms": 200,
        }),
    );
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    let state = wait_for_state(&core, id, |s| s["messages"]["hello"]["count"] == 1).await;
    assert_eq!(state["subprotocol"], "v1.json");
    assert_eq!(
        state["messages"]["hello"]["json"],
        json!({"type": "hello", "n": 1})
    );
    assert_eq!(seen.authorization.lock().unwrap()[0], "Bearer tok");

    let r = core
        .execute(id, "send_text", params(json!({"text": "echo\tme\nplease"})))
        .await;
    assert_eq!(r, Ok(Outcome::Unverified));
    wait_for_state(&core, id, |s| {
        s["last_message"]["text"] == "echo\tme\nplease"
    })
    .await;

    core.execute(id, "send_binary", params(json!({"data": "AAEC/w=="})))
        .await
        .unwrap();
    let s = wait_for_state(&core, id, |s| s["last_message"]["type"] == "binary").await;
    assert_eq!(s["last_message"]["base64"], "AAEC/w==");
    assert_eq!(s["last_message"]["size"], 4);

    // A host file goes as one binary message.
    let path = std::env::temp_dir().join(format!("generic-ws-{}.bin", std::process::id()));
    std::fs::write(&path, [5u8, 6, 7]).unwrap();
    let r = core
        .execute(
            id,
            "send_binary",
            params(json!({"data": {"file": path.to_string_lossy()}})),
        )
        .await;
    std::fs::remove_file(&path).unwrap();
    assert_eq!(r, Ok(Outcome::Unverified));
    wait_for_state(&core, id, |s| s["last_message"]["base64"] == "BQYH").await;

    core.execute(
        id,
        "send_json",
        params(json!({"json": {"type": "cue", "go": 3}})),
    )
    .await
    .unwrap();
    let s = wait_for_state(&core, id, |s| s["messages"]["cue"]["count"] == 1).await;
    assert_eq!(
        s["messages"]["cue"]["json"],
        json!({"type": "cue", "go": 3})
    );

    // The reply is the message carrying the id, not the noise before it.
    let r = core
        .execute(
            id,
            "request",
            params(json!({"json": {"id": 42, "get": "x"}, "json_match": {"$.id": 42}})),
        )
        .await;
    match r {
        Ok(Outcome::Value { value }) => assert_eq!(value["json"], json!({"id": 42, "ok": true})),
        other => panic!("{other:?}"),
    }
    let r = core
        .execute(
            id,
            "request",
            params(json!({"text": "unanswered", "match": "^never", "timeout_ms": 100})),
        )
        .await;
    assert_eq!(r, Err(CommandError::Timeout));

    // Pings are answered, so the device's latency is known.
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().latency_ms.is_none() {
            core.next_events(64).await;
        }
    })
    .await
    .expect("a pong within 10 s");

    // Every message is an event, repeats included.
    core.execute(id, "send_text", params(json!({"text": "same"})))
        .await
        .unwrap();
    core.execute(id, "send_text", params(json!({"text": "same"})))
        .await
        .unwrap();
    let mut repeats = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while repeats < 2 {
            for e in core.next_events(64).await {
                if let Event::Message {
                    device,
                    address,
                    args,
                    source,
                    ..
                } = e
                {
                    if device == id && address == "text" && args[0]["text"] == "same" {
                        assert_eq!(source, format!("127.0.0.1:{port}"));
                        repeats += 1;
                    }
                }
            }
        }
    })
    .await
    .expect("two message events");

    // The device closes: the code is kept and the connection reopened.
    core.execute(id, "send_text", params(json!({"text": "close"})))
        .await
        .unwrap();
    let s = wait_for_state(&core, id, |s| s["close"]["code"] == 4000).await;
    assert_eq!(s["close"]["reason"], "bye");
    wait_for_state(&core, id, |s| s["messages"]["hello"]["count"] == 2).await;
    assert_eq!(seen.connections.load(Ordering::SeqCst), 2);

    // disconnect keeps it closed until connect.
    core.execute(id, "disconnect", params(json!({})))
        .await
        .unwrap();
    wait_for_connection(&core, id, |c| matches!(c, Connection::Disconnected { .. })).await;
    let r = core
        .execute(id, "send_text", params(json!({"text": "x"})))
        .await;
    assert_eq!(r, Err(CommandError::NotConnected));
    core.execute(id, "connect", params(json!({})))
        .await
        .unwrap();
    wait_for_state(&core, id, |s| s["messages"]["hello"]["count"] == 3).await;
    core.close(id).await;
}

fn tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert.der().clone()], key)
    .unwrap();
    tokio_rustls::TlsAcceptor::from(Arc::new(config))
}

#[tokio::test(flavor = "multi_thread")]
async fn wss_with_a_self_signed_certificate_only_when_allowed() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Seen::default());
    let shared = seen.clone();
    let acceptor = tls_acceptor();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let shared = shared.clone();
            tokio::spawn(async move {
                if let Ok(tls) = acceptor.accept(tcp).await {
                    serve(tls, shared).await;
                }
            });
        }
    });
    let core = common::core().unwrap();
    let refused = open(&core, port, json!({"scheme": "wss", "ping_interval_ms": 0}));
    wait_for_connection(
        &core,
        refused,
        |c| matches!(c, Connection::Disconnected { reason } if reason.starts_with("tls:")),
    )
    .await;
    assert_eq!(seen.connections.load(Ordering::SeqCst), 0);
    core.close(refused).await;

    let allowed = open(
        &core,
        port,
        json!({"scheme": "wss", "accept_invalid_certs": true, "key_path": "$.type"}),
    );
    wait_for_state(&core, allowed, |s| s["messages"]["hello"]["count"] == 1).await;
    core.execute(allowed, "send_text", params(json!({"text": "secure"})))
        .await
        .unwrap();
    wait_for_state(&core, allowed, |s| s["last_message"]["text"] == "secure").await;
    core.close(allowed).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unanswered_ping_drops_the_connection() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((tcp, _)) = listener.accept().await {
            // Opened, then never read again: pings go unanswered.
            if let Ok(ws) = tokio_tungstenite::accept_async(tcp).await {
                count.fetch_add(1, Ordering::SeqCst);
                held.push(ws);
            }
        }
    });
    let core = common::core().unwrap();
    let id = open(&core, port, json!({"ping_interval_ms": 150}));
    wait_for_connection(
        &core,
        id,
        |c| matches!(c, Connection::Disconnected { reason } if reason == "no pong within 150 ms"),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while accepted.load(Ordering::SeqCst) < 2 {
            core.next_events(64).await;
        }
    })
    .await
    .expect("reconnected within 10 s");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_credentials_are_not_retried() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let attempts = Arc::new(AtomicUsize::new(0));
    let count = attempts.clone();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            let refuse = |_: &Request, _: Response| -> Result<Response, ErrorResponse> {
                let mut answer = ErrorResponse::new(None);
                *answer.status_mut() = StatusCode::UNAUTHORIZED;
                Err(answer)
            };
            let _ = tokio_tungstenite::accept_hdr_async(tcp, refuse).await;
        }
    });
    let core = common::core().unwrap();
    let id = open(
        &core,
        port,
        json!({"auth": "basic", "username": "u", "password": "wrong"}),
    );
    wait_for_connection(&core, id, |c| matches!(c, Connection::Unauthorized { .. })).await;
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    core.close(id).await;
}
