//! OBS Studio simulated as an obs-websocket 5 server on a real WebSocket,
//! driven through the public API.
//!
//! The simulator requires a password, checks the authentication string the
//! protocol documentation defines, answers the requests the core makes, and
//! emits a scene change event when the program scene is set.

// tungstenite's handshake callback signature returns a large error type.
#![allow(clippy::result_large_err)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

const PASSWORD: &str = "supersecret";
const SALT: &str = "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=";
const CHALLENGE: &str = "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=";

fn expected_authentication() -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{PASSWORD}{SALT}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{CHALLENGE}").as_bytes()))
}

/// Returns the port and the number of connections accepted so far.
async fn simulated_obs() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let callback = |request: &Request, mut response: Response| {
                    let offered = request
                        .headers()
                        .get("Sec-WebSocket-Protocol")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    assert_eq!(offered, "obswebsocket.json");
                    response.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        "obswebsocket.json".parse().unwrap(),
                    );
                    Ok(response)
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(tcp, callback).await else {
                    return;
                };
                let send = |v: Value| Message::text(v.to_string());
                ws.send(send(json!({"op": 0, "d": {
                    "obsWebSocketVersion": "5.5.2",
                    "rpcVersion": 1,
                    "authentication": {"challenge": CHALLENGE, "salt": SALT},
                }})))
                .await
                .unwrap();

                let mut program = "Wide".to_string();
                while let Some(Ok(message)) = ws.next().await {
                    let Message::Text(text) = message else {
                        continue;
                    };
                    let msg: Value = serde_json::from_str(&text).unwrap();
                    let d = &msg["d"];
                    match msg["op"].as_u64() {
                        Some(1) => {
                            if d["authentication"] != expected_authentication() {
                                let _ = ws
                                    .close(Some(CloseFrame {
                                        code: CloseCode::from(4009),
                                        reason: "Authentication failed.".into(),
                                    }))
                                    .await;
                                return;
                            }
                            ws.send(send(json!({"op": 2, "d": {"negotiatedRpcVersion": 1}})))
                                .await
                                .unwrap();
                        }
                        Some(6) => {
                            let kind = d["requestType"].as_str().unwrap();
                            let data = match kind {
                                "GetSceneList" => json!({
                                    "currentProgramSceneName": program,
                                    "currentPreviewSceneName": null,
                                    "scenes": [
                                        {"sceneName": "Close", "sceneIndex": 0},
                                        {"sceneName": "Wide", "sceneIndex": 1},
                                    ],
                                }),
                                "GetStreamStatus" => json!({
                                    "outputActive": false, "outputReconnecting": false,
                                    "outputDuration": 0, "outputCongestion": 0.0,
                                    "outputBytes": 0, "outputSkippedFrames": 0,
                                    "outputTotalFrames": 0,
                                }),
                                "SetCurrentProgramScene" => {
                                    program =
                                        d["requestData"]["sceneName"].as_str().unwrap().to_string();
                                    Value::Null
                                }
                                _ => Value::Null,
                            };
                            let known = matches!(
                                kind,
                                "GetSceneList" | "GetStreamStatus" | "SetCurrentProgramScene"
                            );
                            ws.send(send(json!({"op": 7, "d": {
                                "requestType": kind,
                                "requestId": d["requestId"],
                                "requestStatus": if known {
                                    json!({"result": true, "code": 100})
                                } else {
                                    json!({"result": false, "code": 204, "comment": "Not simulated."})
                                },
                                "responseData": data,
                            }})))
                            .await
                            .unwrap();
                            if kind == "SetCurrentProgramScene" {
                                ws.send(send(json!({"op": 5, "d": {
                                    "eventType": "CurrentProgramSceneChanged",
                                    "eventIntent": 4,
                                    "eventData": {"sceneName": program},
                                }})))
                                .await
                                .unwrap();
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (port, connections)
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

fn open(core: &Core, port: u16, password: &str) -> u64 {
    core.open(OpenRequest {
        device: "obs-studio".into(),
        model: "obs-studio-28".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({ "password": password })),
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn obs_end_to_end() {
    let (port, _) = simulated_obs().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, PASSWORD);

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id && patch["program_scene"] == "Wide")
    })
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.connection, Connection::Connected);
    assert_eq!(snapshot.state["scenes"], json!(["Wide", "Close"]));

    let outcome = core
        .execute(
            id,
            "set_current_program_scene",
            params(json!({"scene_name": "Close"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(
        &core,
        |e| matches!(e, Event::State { patch, .. } if patch["program_scene"] == "Close"),
    )
    .await;

    // A request the simulator refuses comes back as the device's error.
    let outcome = core.execute(id, "start_stream", params(json!({}))).await;
    assert!(matches!(
        outcome,
        Err(CommandError::DeviceError { code: Some(ref c), .. }) if c == "204"
    ));

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn obs_wrong_password_is_unauthorized_and_never_retried() {
    let (port, connections) = simulated_obs().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "wrong");

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
    })
    .await;
    // Well past the first reconnect delay.
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(connections.load(Ordering::SeqCst), 1);
    let outcome = core.execute(id, "start_stream", params(json!({}))).await;
    assert!(matches!(outcome, Err(CommandError::Auth { .. })));
    core.close(id).await;
}
