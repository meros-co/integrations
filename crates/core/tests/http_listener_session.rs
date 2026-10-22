//! The HTTP listener against real loopback sockets, through the public API:
//! webhook senders simulated as HTTP/1.1 and 1.0 clients (RFC 9112).

#![cfg(feature = "http-listener")]

mod common;

use std::time::Duration;

use meros_integrations::{Connection, Core, DeviceId, Event, OpenRequest};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

/// A TCP port free on 127.0.0.1. Loopback only: see `common`.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn open(core: &Core, port: u16, settings: Value) -> DeviceId {
    let mut settings = settings;
    settings["bind_address"] = json!("127.0.0.1");
    core.open(OpenRequest {
        device: "http-listener".into(),
        model: "generic".into(),
        // No host: it hears any sender.
        host: String::new(),
        port: Some(port),
        settings: params(settings),
        monitor: false,
    })
    .unwrap()
}

async fn wait_connected(core: &Core, device: DeviceId) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while core.snapshot(device).unwrap().connection != Connection::Connected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("listening");
}

/// Message events as (address, the request described, source), until `n`
/// have arrived.
async fn messages(core: &Core, n: usize) -> Vec<(String, Value, String)> {
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while out.len() < n {
            for e in core.next_events(1024).await {
                if let Event::Message {
                    address,
                    args,
                    source,
                    ..
                } = e
                {
                    out.push((address, args[0].clone(), source));
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{n} messages, got {out:?}"));
    out
}

/// Read one response with a Content-Length (or none) from the stream.
async fn response(stream: &mut TcpStream) -> String {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(end) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&got[..end]).to_string();
                let length = head
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .map_or(0, |n| n.parse::<usize>().unwrap());
                if got.len() >= end + 4 + length {
                    return;
                }
            }
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0, "closed before the response ended");
            got.extend_from_slice(&buf[..n]);
        }
    })
    .await
    .expect("a response");
    String::from_utf8(got).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn webhooks_are_answered_reported_and_kept_in_state() {
    let core = common::core().unwrap();
    let port = free_port();
    let device = open(
        &core,
        port,
        json!({"response_body": "{\"ok\":true}", "response_content_type": "application/json"}),
    );
    wait_connected(&core, device).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let sender = stream.local_addr().unwrap().to_string();
    let body = r#"{"camera":2,"tally":"program"}"#;
    // Two identical webhooks on one kept-open connection: two events.
    for _ in 0..2 {
        let request = format!(
            "POST /hooks/tally?site=a%20b HTTP/1.1\r\nHost: 127.0.0.1\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let answer = response(&mut stream).await;
        assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
        assert!(answer.contains("Content-Type: application/json\r\n"));
        assert!(answer.ends_with("\r\n\r\n{\"ok\":true}"));
    }
    let got = messages(&core, 2).await;
    for (address, request, source) in &got {
        assert_eq!(address, "/hooks/tally");
        assert_eq!(request["method"], "POST");
        assert_eq!(request["json"], json!({"camera": 2, "tally": "program"}));
        assert_eq!(request["query_params"], json!({"site": "a b"}));
        assert_eq!(source, &sender);
    }
    let state = core.snapshot(device).unwrap().state;
    assert_eq!(state["request_count"], 2);
    assert_eq!(state["paths"]["/hooks/tally"]["json"]["tally"], "program");
    assert_eq!(state["paths"]["/hooks/tally"]["requests"], 2);
    assert_eq!(state["senders"][&sender]["requests"], 2);
    assert_eq!(state["last"]["path"], "/hooks/tally");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_closing_sender_reads_to_the_end_and_credentials_are_checked() {
    let core = common::core().unwrap();
    let port = free_port();
    let device = open(
        &core,
        port,
        json!({"token": "s3cret", "response_status": 204}),
    );
    wait_connected(&core, device).await;

    // Without the token: 401, nothing reported.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(b"GET /event HTTP/1.1\r\nHost: x\r\n\r\n")
        .await
        .unwrap();
    let answer = response(&mut stream).await;
    assert!(
        answer.starts_with("HTTP/1.1 401 Unauthorized\r\n"),
        "{answer}"
    );
    assert!(answer.contains("WWW-Authenticate: Bearer\r\n"));

    // HTTP/1.0: answered, then the sending side shut down, so the sender
    // reads to the end of the stream.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(b"GET /event?n=1 HTTP/1.0\r\nAuthorization: Bearer s3cret\r\n\r\n")
        .await
        .unwrap();
    let mut all = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut all))
        .await
        .expect("the listener ends the stream")
        .unwrap();
    let all = String::from_utf8(all).unwrap();
    assert!(all.starts_with("HTTP/1.1 204 No Content\r\n"), "{all}");
    assert!(all.contains("Connection: close\r\n"));

    let got = messages(&core, 1).await;
    assert_eq!(got[0].0, "/event");
    assert!(got[0].1["headers"].get("authorization").is_none());
    let state = core.snapshot(device).unwrap().state;
    assert_eq!(state["request_count"], 1);
}

#[test]
fn opened_without_a_host_through_the_json_surface() {
    let core = common::core().unwrap();
    let port = free_port();
    let opened = meros_integrations::json::open(
        &core,
        &json!({"device": "http-listener", "model": "generic", "port": port,
                "settings": {"bind_address": "127.0.0.1"}}),
    );
    assert!(opened["device"].is_u64(), "{opened}");
    let refused = meros_integrations::json::open(
        &core,
        &json!({"device": "http-listener", "model": "generic"}),
    );
    assert_eq!(refused["error"]["error"], "not_implemented");
}
