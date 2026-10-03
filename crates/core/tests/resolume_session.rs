//! Resolume Arena (the resolume-push extension) against a simulated Resolume
//! webserver, driven through the public API.
//!
//! As Resolume does, one port serves the REST API and the websocket API
//! (`/api/v1`). On a websocket connecting, the simulator sends the
//! composition, then `sources_update` and `effects_update`; it answers each
//! subscription with the parameter and `type: parameter_subscribed`, and
//! sends the composition again when a layer is added over REST.

#![cfg(feature = "resolume")]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

const LAYER_1: u64 = 1_700_000_001_001;
const LAYER_2: u64 = 1_700_000_001_002;
const LAYER_3: u64 = 1_700_000_001_003;
const COLUMN_1: u64 = 1_700_000_002_001;
const COLUMN_2: u64 = 1_700_000_002_002;
const DECK: u64 = 1_700_000_003_001;

/// A parameter's id: its owner's id with a field number.
fn pid(owner: u64, field: u64) -> u64 {
    owner * 100 + field
}

fn p(owner: u64, field: u64, value: Value) -> Value {
    json!({ "id": pid(owner, field), "value": value })
}

fn clip(id: u64, name: &str, connected: &str) -> Value {
    json!({
        "id": id,
        "name": p(id, 1, json!(name)),
        "connected": { "id": pid(id, 2), "valuetype": "ParamState", "value": connected },
    })
}

fn layer(id: u64, name: &str, clips: [Value; 2]) -> Value {
    json!({
        "id": id,
        "name": p(id, 1, json!(name)),
        "selected": p(id, 2, json!(false)),
        "bypassed": p(id, 3, json!(false)),
        "solo": p(id, 4, json!(false)),
        "master": p(id, 5, json!(1.0)),
        "crossfadergroup": p(id, 6, json!("None")),
        "video": { "opacity": p(id, 7, json!(1.0)) },
        "clips": clips,
        "active_clip": null,
    })
}

#[derive(Default)]
struct Resolume {
    third_layer: bool,
    /// Every parameter path subscribed, in order, over all connections.
    subscribed: Vec<String>,
    /// Parameter values changed since the composition was built, by id.
    changed: BTreeMap<u64, Value>,
}

/// Put the changed values into a composition.
fn apply(v: &mut Value, changed: &BTreeMap<u64, Value>) {
    match v {
        Value::Object(m) => {
            let id = m.get("id").and_then(Value::as_u64);
            if let (Some(new), true) = (id.and_then(|id| changed.get(&id)), m.contains_key("value"))
            {
                m.insert("value".into(), new.clone());
            }
            m.values_mut().for_each(|v| apply(v, changed));
        }
        Value::Array(a) => a.iter_mut().for_each(|v| apply(v, changed)),
        _ => {}
    }
}

impl Resolume {
    fn composition(&self) -> Value {
        let mut layers = vec![
            layer(
                LAYER_1,
                "Background",
                [
                    clip(LAYER_1 + 10, "Clouds", "Connected"),
                    clip(LAYER_1 + 20, "Fire", "Disconnected"),
                ],
            ),
            layer(
                LAYER_2,
                "Overlay",
                [
                    clip(LAYER_2 + 10, "", "Empty"),
                    clip(LAYER_2 + 20, "Logo", "Disconnected"),
                ],
            ),
        ];
        if self.third_layer {
            layers.push(layer(
                LAYER_3,
                "Layer #3",
                [
                    clip(LAYER_3 + 10, "", "Empty"),
                    clip(LAYER_3 + 20, "", "Empty"),
                ],
            ));
        }
        let mut composition = json!({
            "name": p(1, 1, json!("Main Show")),
            "master": p(1, 2, json!(1.0)),
            "speed": p(1, 3, json!(1.0)),
            "bypassed": p(1, 4, json!(false)),
            "video": { "opacity": p(1, 5, json!(1.0)) },
            "crossfader": { "phase": p(1, 6, json!(0.0)), "sidea": { "id": pid(1, 7) },
                            "sideb": { "id": pid(1, 8) } },
            "tempocontroller": { "tempo": p(1, 9, json!(120.0)), "tempo_tap": { "id": pid(1, 10) } },
            "decks": [ { "id": DECK, "closed": false, "name": p(DECK, 1, json!("Deck 1")),
                         "selected": p(DECK, 2, json!(true)) } ],
            "layers": layers,
            "columns": [
                { "id": COLUMN_1, "name": p(COLUMN_1, 1, json!("Intro")),
                  "connected": p(COLUMN_1, 2, json!("Disconnected")), "selected": p(COLUMN_1, 3, json!(true)) },
                { "id": COLUMN_2, "name": p(COLUMN_2, 1, json!("Drop")),
                  "connected": p(COLUMN_2, 2, json!("Disconnected")), "selected": p(COLUMN_2, 3, json!(false)) },
            ],
            "layergroups": [],
        });
        apply(&mut composition, &self.changed);
        composition
    }
}

/// Every parameter's value in a composition, by id.
fn values(v: &Value, out: &mut BTreeMap<u64, Value>) {
    match v {
        Value::Object(m) => {
            if let (Some(id), Some(value)) = (m.get("id").and_then(Value::as_u64), m.get("value")) {
                out.insert(id, value.clone());
            }
            m.values().for_each(|v| values(v, out));
        }
        Value::Array(a) => a.iter().for_each(|v| values(v, out)),
        _ => {}
    }
}

struct Sim {
    port: u16,
    state: Arc<Mutex<Resolume>>,
    /// Messages for every connected websocket.
    push: broadcast::Sender<String>,
}

async fn simulated_resolume() -> Sim {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(Mutex::new(Resolume::default()));
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
                    websocket(tcp, state, push).await;
                } else {
                    http(tcp, state, push).await;
                }
            });
        }
    });
    sim
}

/// The request head, without consuming it.
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

async fn websocket(tcp: TcpStream, state: Arc<Mutex<Resolume>>, push: broadcast::Sender<String>) {
    let mut pushed = push.subscribe();
    let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else {
        return;
    };
    let composition = state.lock().unwrap().composition();
    for message in [
        composition,
        json!({"type": "sources_update", "value": {"video": []}}),
        json!({"type": "effects_update", "value": {"video": []}}),
    ] {
        ws.send(Message::text(message.to_string())).await.unwrap();
    }
    loop {
        tokio::select! {
            message = ws.next() => {
                let Some(Ok(Message::Text(text))) = message else { return };
                let msg: Value = serde_json::from_str(&text).unwrap();
                let path = msg["parameter"].as_str().unwrap_or_default().to_string();
                if msg["action"] != "subscribe" {
                    continue;
                }
                let id: u64 = path.strip_prefix("/parameter/by-id/").unwrap().parse().unwrap();
                let value = {
                    let mut s = state.lock().unwrap();
                    s.subscribed.push(path.clone());
                    let mut all = BTreeMap::new();
                    values(&s.composition(), &mut all);
                    all.get(&id).cloned()
                };
                let Some(value) = value else { continue };
                let reply = json!({"type": "parameter_subscribed", "path": path, "id": id, "value": value});
                ws.send(Message::text(reply.to_string())).await.unwrap();
            }
            message = pushed.recv() => {
                let Ok(text) = message else { return };
                if ws.send(Message::text(text)).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn http(mut tcp: TcpStream, state: Arc<Mutex<Resolume>>, push: broadcast::Sender<String>) {
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
    let mut first = head.lines().next().unwrap_or_default().split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let (status, body) = match (method, target) {
        ("GET", "/api/v1/product") => (
            "200 OK",
            json!({"name": "Arena", "major": 7, "minor": 23, "micro": 0, "revision": 1})
                .to_string(),
        ),
        ("GET", "/api/v1/composition") => {
            ("200 OK", state.lock().unwrap().composition().to_string())
        }
        ("POST", "/api/v1/composition/layers/add") => {
            let composition = {
                let mut s = state.lock().unwrap();
                s.third_layer = true;
                s.composition()
            };
            // A structural change: the composition goes to every websocket.
            let _ = push.send(composition.to_string());
            ("204 No Content", String::new())
        }
        _ => ("404 Not Found", String::new()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = tcp.write_all(response.as_bytes()).await;
    let _ = tcp.shutdown().await;
}

/// Wait until the device's state satisfies `pred`.
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

async fn subscriptions_where(sim: &Sim, what: &str, pred: impl Fn(&[String]) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&sim.state.lock().unwrap().subscribed) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("subscriptions within 10 s: {what}"))
}

fn by_id(id: u64) -> String {
    format!("/parameter/by-id/{id}")
}

#[tokio::test(flavor = "multi_thread")]
async fn resolume_state_is_pushed() {
    let sim = simulated_resolume().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "resolume".into(),
            model: "arena".into(),
            host: "127.0.0.1".into(),
            port: Some(sim.port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();

    // The composition sent on connecting.
    let state = state_where(&core, id, "initial composition", |s| {
        s["layers"][LAYER_2.to_string()]["name"] == "Overlay"
    })
    .await;
    let l1 = &state["layers"][LAYER_1.to_string()];
    assert_eq!(l1["position"], 1);
    assert_eq!(l1["opacity"], 1.0);
    assert_eq!(l1["active_clip"], LAYER_1 + 10);
    assert_eq!(l1["active_clip_name"], "Clouds");
    assert_eq!(
        state["clips"][(LAYER_1 + 20).to_string()],
        json!({"layer": LAYER_1, "column": COLUMN_2, "name": "Fire", "connected": "Disconnected"})
    );
    assert_eq!(state["columns"][COLUMN_1.to_string()]["name"], "Intro");
    assert_eq!(
        state["decks"][DECK.to_string()],
        json!({"position": 1, "name": "Deck 1", "selected": true, "closed": false})
    );
    assert_eq!(state["composition"]["name"], "Main Show");
    assert_eq!(state["tempo"]["tap_parameter"], pid(1, 10));
    // composition 7, decks 2, layers 2x7, clips 4x2, columns 2x3.
    const FIRST: usize = 7 + 2 + 14 + 8 + 6;
    subscriptions_where(&sim, "every parameter", |s| s.len() == FIRST).await;
    {
        let subscribed = &sim.state.lock().unwrap().subscribed;
        assert!(subscribed.contains(&by_id(pid(LAYER_1, 7))));
        assert!(subscribed.contains(&by_id(pid(LAYER_2 + 20, 2))));
        // Event parameters are only ids.
        assert!(!subscribed.contains(&by_id(pid(1, 10))));
    }
    // The product, from the probe.
    state_where(&core, id, "product", |s| s["product"]["name"] == "Arena").await;

    // Parameter updates: Resolume's value changes, and it tells subscribers.
    let update = |param: u64, value: Value| {
        sim.state
            .lock()
            .unwrap()
            .changed
            .insert(param, value.clone());
        let message =
            json!({"type": "parameter_update", "path": by_id(param), "id": param, "value": value});
        sim.push.send(message.to_string()).unwrap();
    };
    update(pid(LAYER_1, 7), json!(0.25));
    update(pid(LAYER_1 + 20, 2), json!("Connected"));
    update(pid(LAYER_1 + 10, 2), json!("Disconnected"));
    let state = state_where(&core, id, "updates", |s| {
        s["layers"][LAYER_1.to_string()]["opacity"] == 0.25
            && s["layers"][LAYER_1.to_string()]["active_clip"] == LAYER_1 + 20
    })
    .await;
    assert_eq!(
        state["layers"][LAYER_1.to_string()]["active_clip_name"],
        "Fire"
    );
    assert_eq!(
        state["clips"][(LAYER_1 + 10).to_string()]["connected"],
        "Disconnected"
    );

    // A layer added over REST: Resolume sends the composition again, and
    // only the new layer is subscribed.
    let outcome = core.execute(id, "add_layer", Default::default()).await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    let state = state_where(&core, id, "third layer", |s| {
        s["layers"][LAYER_3.to_string()]["name"] == "Layer #3"
    })
    .await;
    assert_eq!(state["layers"][LAYER_3.to_string()]["position"], 3);
    assert_eq!(
        state["clips"][(LAYER_3 + 20).to_string()]["connected"],
        "Empty"
    );
    assert_eq!(state["layers"][LAYER_1.to_string()]["opacity"], 0.25);
    subscriptions_where(&sim, "the new layer", |s| s.len() == FIRST + 7 + 4).await;
    let mut new: Vec<String> = (1..=7).map(|f| by_id(pid(LAYER_3, f))).collect();
    for clip in [LAYER_3 + 10, LAYER_3 + 20] {
        new.extend([by_id(pid(clip, 1)), by_id(pid(clip, 2))]);
    }
    new.sort();
    let mut added = sim.state.lock().unwrap().subscribed[FIRST..].to_vec();
    added.sort();
    assert_eq!(added, new);

    // And the new layer's parameters update.
    update(pid(LAYER_3, 1), json!("Titles"));
    state_where(&core, id, "new layer renamed", |s| {
        s["layers"][LAYER_3.to_string()]["name"] == "Titles"
    })
    .await;

    core.close(id).await;
}
