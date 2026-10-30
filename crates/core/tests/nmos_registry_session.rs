//! An NMOS registry (nmos-registry, with the nmos-query-websocket extension)
//! against a simulated IS-04 Query API, driven through the public API.
//!
//! As a registry does (QueryAPI.raml, "Behaviour - Querying"), the simulator
//! answers `POST /subscriptions` with a `ws_href` on its own port; each
//! websocket first sends the whole list of its resource type (the sync
//! grain), then the changes the test pushes. Closing a websocket from the
//! registry's side makes the core subscribe again.

#![cfg(feature = "nmos-registry")]

mod common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{json, value::Value};
use meros_integrations::{Connection, Core, OpenRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

const CAM: &str = "5709255c-c0ae-4e1e-99a0-e872e83e48e0";
const ISO: &str = "6c1e2a9b-3f4d-4e5a-8b6c-7d8e9f0a1b2c";

fn sender(id: &str, label: &str) -> Value {
    json!({
        "id": id, "version": "1:1", "label": label, "description": "", "tags": {},
        "flow_id": null, "transport": "urn:x-nmos:transport:rtp.mcast",
        "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "manifest_href": null,
        "interface_bindings": [], "subscription": {"receiver_id": null, "active": false}
    })
}

fn grain(topic: &str, data: Value) -> String {
    json!({
        "grain_type": "event", "source_id": "x", "flow_id": "y",
        "origin_timestamp": "0:0", "sync_timestamp": "0:0", "creation_timestamp": "0:0",
        "rate": {"numerator": 0, "denominator": 1}, "duration": {"numerator": 0, "denominator": 1},
        "grain": {"type": "urn:x-nmos:format:data.event", "topic": topic, "data": data}
    })
    .to_string()
}

#[derive(Default)]
struct Registry {
    /// The resource_path of every subscription created, in order.
    created: Vec<String>,
    senders: Vec<Value>,
}

struct Sim {
    port: u16,
    state: Arc<Mutex<Registry>>,
    /// Text for every open senders websocket; "close" closes them.
    push: broadcast::Sender<String>,
}

async fn simulated_registry() -> Sim {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(Mutex::new(Registry {
        senders: vec![sender(CAM, "Camera 1")],
        ..Default::default()
    }));
    let (push, _) = broadcast::channel(64);
    let sim = Sim {
        port,
        state: state.clone(),
        push: push.clone(),
    };
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let state = state.clone();
            let push = push.clone();
            tokio::spawn(async move {
                let Some(head) = peek_head(&tcp).await else {
                    return;
                };
                if head.to_ascii_lowercase().contains("upgrade: websocket") {
                    let path = head.split(' ').nth(1).unwrap_or_default().to_string();
                    websocket(tcp, &path, state, push).await;
                } else {
                    http(tcp, port, state).await;
                }
            });
        }
    });
    sim
}

async fn peek_head(tcp: &TcpStream) -> Option<String> {
    let mut buf = vec![0u8; 8192];
    for _ in 0..200 {
        let n = tcp.peek(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        if let Some(end) = buf[..n].windows(4).position(|w| w == b"\r\n\r\n") {
            return Some(String::from_utf8_lossy(&buf[..end]).into_owned());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    None
}

/// `/x-nmos/query/v1.3/subscriptions/<type>`: this simulator names each
/// subscription after its resource type.
async fn websocket(
    tcp: TcpStream,
    path: &str,
    state: Arc<Mutex<Registry>>,
    push: broadcast::Sender<String>,
) {
    let kind = path.rsplit('/').next().unwrap_or_default().to_string();
    let mut pushed = push.subscribe();
    let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else {
        return;
    };
    // The sync grain: every resource, pre and post alike.
    let items: Vec<Value> = if kind == "senders" {
        state.lock().unwrap().senders.clone()
    } else {
        Vec::new()
    };
    let data: Vec<Value> = items
        .iter()
        .map(|s| json!({"path": s["id"], "pre": s, "post": s}))
        .collect();
    let topic = format!("/{kind}/");
    ws.send(Message::text(grain(&topic, json!(data))))
        .await
        .unwrap();
    loop {
        tokio::select! {
            message = ws.next() => {
                if !matches!(message, Some(Ok(_))) { return; }
            }
            message = pushed.recv() => {
                let Ok(text) = message else { return };
                if kind != "senders" { continue; }
                if text == "close" {
                    let _ = ws.close(None).await;
                    return;
                }
                if ws.send(Message::text(text)).await.is_err() { return; }
            }
        }
    }
}

async fn http(mut tcp: TcpStream, port: u16, state: Arc<Mutex<Registry>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
        let Ok(n) = tcp.read(&mut chunk).await else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let length: usize = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse().ok())?
        })
        .unwrap_or(0);
    while buf.len() < head_end + length {
        let Ok(n) = tcp.read(&mut chunk).await else {
            return;
        };
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body: Value = serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
    let mut first = head.lines().next().unwrap_or_default().split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let (status, reply) = match (method, target) {
        ("GET", "/x-nmos/query") => ("200 OK", json!(["v1.3/"])),
        ("POST", "/x-nmos/query/v1.3/subscriptions") => {
            let resource = body["resource_path"].as_str().unwrap_or("").to_string();
            assert_eq!(body["persist"], false);
            assert_eq!(body["max_update_rate_ms"], 100);
            state.lock().unwrap().created.push(resource.clone());
            let kind = resource.trim_start_matches('/');
            (
                "201 Created",
                json!({
                    "id": "8f2c1a2b-0000-4000-8000-000000000001",
                    "ws_href": format!("ws://127.0.0.1:{port}/x-nmos/query/v1.3/subscriptions/{kind}"),
                    "max_update_rate_ms": 100, "persist": false, "secure": false,
                    "resource_path": resource, "params": {}
                }),
            )
        }
        _ => (
            "404 Not Found",
            json!({"code": 404, "error": "not found", "debug": null}),
        ),
    };
    let reply = reply.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = tcp.write_all(response.as_bytes()).await;
    let _ = tcp.shutdown().await;
}

async fn state_where(core: &Core, id: u64, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(s) = core.snapshot(id) {
                if pred(&s.state) {
                    return s.state;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "state within 10 s: {what}; last {:?}",
            core.snapshot(id).map(|s| s.state)
        )
    })
}

async fn created_where(sim: &Sim, what: &str, pred: impl Fn(&[String]) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&sim.state.lock().unwrap().created) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("subscriptions within 10 s: {what}"))
}

fn open(core: &Core, port: u16, monitor: bool) -> u64 {
    core.open(OpenRequest {
        device: "nmos-registry".into(),
        model: "registry".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: Default::default(),
        monitor,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_registry_is_followed_through_its_subscriptions() {
    let sim = simulated_registry().await;
    let core = common::core().unwrap();
    let id = open(&core, sim.port, true);

    // One subscription per resource type.
    created_where(&sim, "six subscriptions", |c| c.len() == 6).await;
    let mut created = sim.state.lock().unwrap().created.clone();
    created.sort();
    assert_eq!(
        created,
        [
            "/devices",
            "/flows",
            "/nodes",
            "/receivers",
            "/senders",
            "/sources"
        ]
    );

    // The sync grain fills state.
    let state = state_where(&core, id, "the sync", |s| {
        s["senders"][CAM]["label"] == "Camera 1"
    })
    .await;
    assert_eq!(state["senders"][CAM]["flow_id"], "");
    assert_eq!(state["senders"][CAM]["subscription"]["active"], false);

    // A change and an addition.
    let iso = sender(ISO, "ISO 1");
    let changed = sender(CAM, "Camera 1 wide");
    sim.push
        .send(grain(
            "/senders/",
            json!([
                {"path": CAM, "pre": sender(CAM, "Camera 1"), "post": changed},
                {"path": ISO, "post": iso},
            ]),
        ))
        .unwrap();
    state_where(&core, id, "changed and added", |s| {
        s["senders"][CAM]["label"] == "Camera 1 wide" && s["senders"][ISO]["label"] == "ISO 1"
    })
    .await;

    // A deletion.
    sim.push
        .send(grain(
            "/senders/",
            json!([{"path": ISO, "pre": sender(ISO, "ISO 1")}]),
        ))
        .unwrap();
    state_where(&core, id, "deleted", |s| s["senders"].get(ISO).is_none()).await;

    // The registry drops the websocket: the core subscribes again, and the
    // new sync (Camera 1 gone, ISO 2 there) replaces the list.
    {
        let mut s = sim.state.lock().unwrap();
        s.senders = vec![sender(ISO, "ISO 2")];
    }
    sim.push.send("close".into()).unwrap();
    created_where(&sim, "senders again", |c| {
        c.iter().filter(|r| *r == "/senders").count() == 2
    })
    .await;
    let state = state_where(&core, id, "the new sync", |s| {
        s["senders"][ISO]["label"] == "ISO 2"
    })
    .await;
    assert!(state["senders"].get(CAM).is_none());

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_registry_opened_for_commands_only_is_not_subscribed() {
    let sim = simulated_registry().await;
    let core = common::core().unwrap();
    let id = open(&core, sim.port, false);
    // The probe answers, so the device connects; nothing subscribes.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core
                .snapshot(id)
                .is_some_and(|s| s.connection == Connection::Connected)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("connected within 10 s");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(sim.state.lock().unwrap().created.is_empty());
    core.close(id).await;
}
