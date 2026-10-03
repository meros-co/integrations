//! The generic OSC, TCP/UDP and HTTP modules against real local sockets and a
//! real HTTP server, driven through the public API.
//!
//! Each peer is a small simulator: an OSC device answering `/info` and
//! pushing to a configured feedback port (OSC 1.0 over UDP, OSC 1.1 SLIP over
//! TCP), a line device answering `PWR?` and closing on `BYE`, and an HTTP/1.1
//! server (RFC 9110) answering a status document, an echo and a 404.

#![cfg(all(
    feature = "generic-http",
    feature = "generic-osc",
    feature = "generic-tcp-udp"
))]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, DeviceId, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, device: &str, port: u16, settings: Value) -> DeviceId {
    core.open(OpenRequest {
        device: device.into(),
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

async fn wait_for_connection(core: &Core, id: DeviceId, want: fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if want(&core.snapshot(id).unwrap().connection) {
                return;
            }
            for event in core.next_events(64).await {
                if let Event::Connection { device, connection } = event {
                    if device == id && want(&connection) {
                        return;
                    }
                }
            }
        }
    })
    .await
    .expect("connection state within 10 s")
}

fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

// --- OSC ------------------------------------------------------------------

fn osc_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// An OSC message with string arguments only.
fn osc(address: &str, strings: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, address);
    osc_string(&mut out, &format!(",{}", "s".repeat(strings.len())));
    for s in strings {
        osc_string(&mut out, s);
    }
    out
}

fn address_of(packet: &[u8]) -> String {
    let end = packet.iter().position(|b| *b == 0).unwrap_or(packet.len());
    String::from_utf8_lossy(&packet[..end]).into_owned()
}

/// Answers `/info` with `/info "sim" "1.0"`; after `/subscribe`, pushes
/// `/fader/1 "up"` to `feedback` (or to the sender when there is none).
async fn osc_udp_device(feedback: Option<u16>) -> (u16, Arc<Mutex<Vec<Vec<u8>>>>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let received: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let log = received.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let packet = buf[..n].to_vec();
            log.lock().unwrap().push(packet.clone());
            match address_of(&packet).as_str() {
                "/info" => {
                    let _ = socket.send_to(&osc("/info", &["sim", "1.0"]), from).await;
                }
                "/subscribe" => {
                    let to = feedback.map_or(from, |p| SocketAddr::from(([127, 0, 0, 1], p)));
                    let _ = socket.send_to(&osc("/fader/1", &["up"]), to).await;
                }
                _ => {}
            }
        }
    });
    (port, received)
}

#[tokio::test(flavor = "multi_thread")]
async fn osc_over_udp() {
    let (port, received) = osc_udp_device(None).await;
    let core = Core::new().unwrap();
    let id = open(&core, "generic-osc", port, json!({}));
    wait_for_connection(&core, id, |c| *c == Connection::Unmonitored).await;

    // OSC 1.0 "Examples": /foo 1000 -1 "hello" 1.234 5.678, byte for byte.
    let sent = core
        .execute(
            id,
            "send",
            params(json!({"address": "/foo", "args": [1000, -1, "hello", 1.234, 5.678]})),
        )
        .await;
    assert_eq!(sent, Ok(Outcome::Unverified));

    let reply = core
        .execute(id, "query", params(json!({"address": "/info"})))
        .await;
    assert_eq!(
        reply,
        Ok(Outcome::Value {
            value: json!({"address": "/info", "types": "ss", "args": ["sim", "1.0"]})
        })
    );
    let foo: Vec<u8> = [
        "2f666f6f", "00000000", "2c696973", "66660000", "000003e8", "ffffffff", "68656c6c",
        "6f000000", "3f9df3b6", "40b5b22d",
    ]
    .concat()
    .as_bytes()
    .chunks(2)
    .map(|h| u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap())
    .collect();
    assert_eq!(received.lock().unwrap()[0], foo);

    // Replies to the sender are kept in state.
    core.execute(id, "send", params(json!({"address": "/subscribe"})))
        .await
        .unwrap();
    let state = wait_for_state(&core, id, |s| {
        s["messages"]["/fader/1"]["args"] == json!(["up"])
    })
    .await;
    assert_eq!(state["messages"]["/info"]["args"], json!(["sim", "1.0"]));
    assert_eq!(state["last_message"]["address"], "/fader/1");

    // No reply on the address asked for: a timeout.
    let none = core
        .execute(
            id,
            "query",
            params(json!({"address": "/info", "reply_address": "/never", "timeout_ms": 200})),
        )
        .await;
    assert_eq!(none, Err(CommandError::Timeout));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn osc_feedback_to_a_fixed_listen_port() {
    let listen = free_udp_port();
    let (port, _) = osc_udp_device(Some(listen)).await;
    let core = Core::new().unwrap();
    let id = open(&core, "generic-osc", port, json!({"listen_port": listen}));
    wait_for_connection(&core, id, |c| *c == Connection::Unmonitored).await;
    core.execute(id, "send", params(json!({"address": "/subscribe"})))
        .await
        .unwrap();
    wait_for_state(&core, id, |s| {
        s["messages"]["/fader/1"]["args"] == json!(["up"])
    })
    .await;
    core.close(id).await;
}

fn slip(packet: &[u8]) -> Vec<u8> {
    let mut out = vec![0xC0];
    for &b in packet {
        match b {
            0xC0 => out.extend_from_slice(&[0xDB, 0xDC]),
            0xDB => out.extend_from_slice(&[0xDB, 0xDD]),
            b => out.push(b),
        }
    }
    out.push(0xC0);
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn osc_over_tcp_with_slip() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut pending = Vec::new();
        let mut buf = [0u8; 2048];
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            // Packets between END bytes (OSC 1.1: double-ended).
            while let Some(end) = pending.iter().skip(1).position(|b| *b == 0xC0) {
                let frame: Vec<u8> = pending.drain(..end + 2).collect();
                let packet = &frame[1..frame.len() - 1];
                if address_of(packet) == "/eos/get/version" {
                    let reply = slip(&osc("/eos/out/get/version", &["3.2.0"]));
                    stream.write_all(&reply).await.unwrap();
                }
            }
        }
    });

    let core = Core::new().unwrap();
    let id = open(&core, "generic-osc", port, json!({"transport": "tcp"}));
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    let reply = core
        .execute(
            id,
            "query",
            params(json!({"address": "/eos/get/version", "reply_address": "/eos/out/get/version"})),
        )
        .await;
    assert_eq!(
        reply,
        Ok(Outcome::Value {
            value: json!({"address": "/eos/out/get/version", "types": "s", "args": ["3.2.0"]})
        })
    );
    core.close(id).await;
}

// --- TCP / UDP ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn tcp_lines_requests_and_reconnection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicUsize::new(0));
    let received: Arc<Mutex<Vec<u8>>> = Arc::default();
    let (count, log) = (connections.clone(), received.clone());
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(serve_lines(stream, log.clone()));
        }
    });

    let core = Core::new().unwrap();
    let id = open(
        &core,
        "generic-tcp-udp",
        port,
        json!({"send_terminator": "cr"}),
    );
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;

    let reply = core
        .execute(
            id,
            "request",
            params(json!({"text": "PWR?", "match": "^PWR="})),
        )
        .await;
    assert_eq!(
        reply,
        Ok(Outcome::Value {
            value: json!({"text": "PWR=1", "hex": "5057523d31"})
        })
    );
    let state = wait_for_state(&core, id, |s| s["received_count"] == 2).await;
    assert_eq!(state["recent"][0]["text"], "HELLO");
    assert_eq!(state["last_message"]["text"], "PWR=1");

    assert_eq!(
        core.execute(id, "send_hex", params(json!({"hex": "02 41 03"})))
            .await,
        Ok(Outcome::Unverified)
    );
    assert_eq!(
        core.execute(id, "send_text", params(json!({"text": r"A\tB\x21"})))
            .await,
        Ok(Outcome::Unverified)
    );

    // The device closes the connection; the module connects again.
    core.execute(id, "send_text", params(json!({"text": "BYE"})))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while connections.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("reconnected within 10 s");
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    let again = core
        .execute(
            id,
            "request",
            params(json!({"text": "PWR?", "match": "^PWR"})),
        )
        .await;
    assert!(matches!(again, Ok(Outcome::Value { .. })), "{again:?}");

    let bytes = received.lock().unwrap().clone();
    let expect: &[u8] = b"PWR?\r\x02A\x03A\tB!\rBYE\r";
    assert!(
        bytes.windows(expect.len()).any(|w| w == expect),
        "{:?}",
        String::from_utf8_lossy(&bytes)
    );
    core.close(id).await;
}

/// Greets with `HELLO`, answers `PWR?` with `PWR=1`, closes on `BYE`.
async fn serve_lines(mut stream: TcpStream, log: Arc<Mutex<Vec<u8>>>) {
    stream.write_all(b"HELLO\r\n").await.unwrap();
    let mut pending = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        log.lock().unwrap().extend_from_slice(&buf[..n]);
        pending.extend_from_slice(&buf[..n]);
        while let Some(end) = pending.iter().position(|b| *b == b'\r') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            match &line[..line.len() - 1] {
                b"PWR?" => stream.write_all(b"PWR=1\r\n").await.unwrap(),
                b"BYE" => return,
                _ => {}
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn udp_requests() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            if &buf[..n] == b"\x81\x09\x04\x00\xff" {
                // A VISCA-style binary answer, no terminator.
                let _ = socket.send_to(b"\x90\x50\x02\xff", from).await;
            }
        }
    });
    let core = Core::new().unwrap();
    let id = open(
        &core,
        "generic-tcp-udp",
        port,
        json!({"protocol": "udp", "terminator": "none"}),
    );
    wait_for_connection(&core, id, |c| *c == Connection::Unmonitored).await;
    let reply = core
        .execute(
            id,
            "request",
            params(json!({"hex": "81 09 04 00 FF", "timeout_ms": 2000})),
        )
        .await;
    assert_eq!(
        reply,
        Ok(Outcome::Value {
            value: json!({"text": "\u{fffd}P\u{2}\u{fffd}", "hex": "905002ff"})
        })
    );
    let state = wait_for_state(&core, id, |s| s["last_message"]["hex"] == "905002ff").await;
    assert_eq!(state["received_count"], 1);
    core.close(id).await;
}

// --- HTTP -----------------------------------------------------------------

struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn read_request(stream: &mut TcpStream) -> Option<Seen> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some(Seen {
        method,
        path,
        headers,
        body,
    })
}

/// One response per connection, then close.
async fn http_server() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let polls = Arc::new(AtomicUsize::new(0));
    let count = polls.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let count = count.clone();
            tokio::spawn(async move {
                let Some(req) = read_request(&mut stream).await else {
                    return;
                };
                let header = |name: &str| {
                    req.headers
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, v)| v.clone())
                };
                let (status, body) = match req.path.as_str() {
                    "/api/status" => {
                        let n = count.fetch_add(1, Ordering::SeqCst) + 1;
                        ("200 OK", json!({"power": "on", "polls": n}).to_string())
                    }
                    "/api/echo?x=1" => (
                        "200 OK",
                        json!({
                            "method": req.method,
                            "authorization": header("authorization"),
                            "content_type": header("content-type"),
                            "x_custom": header("x-custom"),
                            "body": String::from_utf8_lossy(&req.body),
                        })
                        .to_string(),
                    ),
                    "/text" => ("200 OK", "plain words".to_string()),
                    _ => ("404 Not Found", "no such resource".to_string()),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (port, polls)
}

#[tokio::test(flavor = "multi_thread")]
async fn http_requests_and_polling() {
    let (port, polls) = http_server().await;
    let core = Core::new().unwrap();
    let id = open(
        &core,
        "generic-http",
        port,
        json!({
            "auth": "basic", "username": "admin", "password": "secret",
            "headers": {"X-Custom": "from-settings"},
            "poll_path": "/api/status", "poll_interval_ms": 200
        }),
    );

    // The poll lands in state, and repeats.
    let state = wait_for_state(&core, id, |s| {
        s["poll"]["json"]["polls"].as_u64() >= Some(2)
    })
    .await;
    assert_eq!(state["poll"]["status"], 200);
    assert_eq!(state["poll"]["ok"], true);
    assert_eq!(state["poll"]["json"]["power"], "on");
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;

    let echo = core
        .execute(
            id,
            "request",
            params(json!({
                "method": "PATCH", "path": "/api/echo?x=1",
                "headers": {"x-custom": "from-request"}, "body": {"level": -6}
            })),
        )
        .await;
    let Ok(Outcome::Value { value }) = echo else {
        panic!("{echo:?}")
    };
    assert_eq!(value["status"], 200);
    assert_eq!(
        value["json"],
        json!({
            "method": "PATCH",
            // RFC 7617 §2: base64 of "admin:secret".
            "authorization": "Basic YWRtaW46c2VjcmV0",
            "content_type": "application/json",
            "x_custom": "from-request",
            "body": "{\"level\":-6}",
        })
    );

    let text = core
        .execute(id, "request", params(json!({"path": "/text"})))
        .await;
    assert_eq!(
        text,
        Ok(Outcome::Value {
            value: json!({"status": 200, "ok": true, "body": "plain words"})
        })
    );

    let missing = core
        .execute(id, "request", params(json!({"path": "/nope"})))
        .await;
    assert_eq!(
        missing,
        Err(CommandError::DeviceError {
            code: Some("404".into()),
            message: "HTTP 404: no such resource".into()
        })
    );
    let tolerated = core
        .execute(
            id,
            "request",
            params(json!({"method": "DELETE", "path": "/nope", "fail_on_status": false})),
        )
        .await;
    assert!(
        matches!(&tolerated, Ok(Outcome::Value { value }) if value["status"] == 404 && value["ok"] == false),
        "{tolerated:?}"
    );

    let now = core.execute(id, "poll_now", params(json!({}))).await;
    assert!(
        matches!(&now, Ok(Outcome::Value { value }) if value["json"]["power"] == "on"),
        "{now:?}"
    );
    assert!(polls.load(Ordering::SeqCst) >= 3);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn http_without_a_server_reports_disconnected() {
    // A port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let core = Core::new().unwrap();
    let id = open(&core, "generic-http", port, json!({}));
    wait_for_connection(&core, id, |c| *c == Connection::Unmonitored).await;
    let result = core
        .execute(id, "request", params(json!({"path": "/"})))
        .await;
    assert!(
        matches!(result, Err(CommandError::Transport { .. })),
        "{result:?}"
    );
    wait_for_connection(&core, id, |c| matches!(c, Connection::Disconnected { .. })).await;
    core.close(id).await;
}
