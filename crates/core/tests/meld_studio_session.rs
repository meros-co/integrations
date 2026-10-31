//! Meld Studio simulated as a WebChannel server on a real WebSocket, driven
//! through the public API.
//!
//! The simulator answers the init request with the meld object's methods,
//! signals and properties, sends property updates only after the client says
//! it is idle, answers invokes, and switches scenes when showScene is
//! invoked, sending the new session as a property update.

#![cfg(feature = "meld-studio")]

mod common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

const SCENE_A: &str = "05731BDA0C5DE934359C8F7F24CE6C1B";
const SCENE_B: &str = "9DA5A633182EB0C0804E671B26A1B581";
const TRACK: &str = "D4D3457F8AF5D1F5381A903A8C43BE27";

fn session(current: &str) -> Value {
    json!({"items": {
        SCENE_A: {"type": "scene", "index": 0, "name": "Intro", "current": current == SCENE_A, "staged": false},
        SCENE_B: {"type": "scene", "index": 1, "name": "Main", "current": current == SCENE_B, "staged": false},
        TRACK: {"type": "track", "name": "Microphone", "muted": false, "monitoring": false},
    }})
}

/// Returns the port and every invoke the simulator received, as
/// (method index, args).
async fn simulated_meld() -> (u16, Arc<Mutex<Vec<(u64, Value)>>>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let invokes = Arc::new(Mutex::new(Vec::new()));
    let seen = invokes.clone();
    tokio::spawn(async move {
        let Ok((tcp, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else {
            return;
        };
        let mut current = SCENE_A.to_string();
        // A property update waiting for the client to say it is idle.
        let mut queued: Option<Value> = None;
        let mut idle = false;
        while let Some(Ok(message)) = ws.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            let msg: Value = serde_json::from_str(&text).unwrap();
            let reply = match msg["type"].as_u64() {
                Some(3) => Some(json!({"type": 10, "id": msg["id"], "data": {"meld": {
                    "methods": [["showScene", 10], ["registerTrackObserver", 19], ["toggleRecord", 17]],
                    "signals": [["gainUpdated", 4], ["sessionChanged", 7]],
                    "properties": [
                        [1, "isStreaming", [1, 5], false],
                        [2, "isRecording", [1, 6], false],
                        [3, "session", [1, 7], session(&current)],
                        [4, "version", [1, 8], 2],
                    ],
                    "enums": {},
                }}})),
                Some(4) => {
                    idle = true;
                    None
                }
                Some(6) => {
                    let method = msg["method"].as_u64().unwrap();
                    seen.lock().unwrap().push((method, msg["args"].clone()));
                    if method == 10 {
                        current = msg["args"][0].as_str().unwrap().to_string();
                        queued = Some(json!({"type": 2, "data": [
                            {"object": "meld", "signals": {"7": []}, "properties": {"3": session(&current)}}
                        ]}));
                    }
                    if method == 19 {
                        // Registered: the track's gain follows.
                        ws.send(Message::text(
                            json!({"type": 1, "object": "meld", "signal": 4, "args": [TRACK, 0.75, false]})
                                .to_string(),
                        ))
                        .await
                        .unwrap();
                    }
                    Some(json!({"type": 10, "id": msg["id"], "data": null}))
                }
                _ => None,
            };
            if let Some(reply) = reply {
                ws.send(Message::text(reply.to_string())).await.unwrap();
            }
            if idle {
                if let Some(update) = queued.take() {
                    idle = false;
                    ws.send(Message::text(update.to_string())).await.unwrap();
                }
            }
        }
    });
    (port, invokes)
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event within 10 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn meld_end_to_end() {
    let (port, invokes) = simulated_meld().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "meld-studio".into(),
            model: "meld-studio".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id && patch["current_scene"] == SCENE_A)
    })
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.connection, Connection::Connected);
    assert_eq!(snapshot.state["api_version"], 2);
    assert_eq!(snapshot.state["scenes"][SCENE_B]["name"], "Main");

    let outcome = core
        .execute(id, "show_scene", params(json!({"scene": SCENE_B})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(
        &core,
        |e| matches!(e, Event::State { patch, .. } if patch["current_scene"] == SCENE_B),
    )
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.state["scenes"][SCENE_A]["current"], false);
    assert_eq!(snapshot.state["tracks"][TRACK]["gain"], 0.75);
    assert!(invokes
        .lock()
        .unwrap()
        .contains(&(19, json!(["meros", TRACK]))));

    // A method this Meld does not offer is the device's refusal.
    let outcome = core
        .execute(id, "toggle_streaming", params(json!({})))
        .await;
    assert!(outcome.is_err());

    core.close(id).await;
}
