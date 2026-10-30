//! Tally Arbiter simulated as a Socket.IO 4 server (Engine.IO 4 over a
//! WebSocket on /socket.io/) bound to 127.0.0.1, driven through the public
//! API.
//!
//! The simulator speaks the events Tally Arbiter's server handles: it signs a
//! user in and answers with an access token, joins the settings room with its
//! initialdata, registers a listener client and pushes that device's busses,
//! answers reads with the event of the same name, and answers manage events
//! with manage_response.

#![cfg(feature = "tally-arbiter")]
// tungstenite's handshake callback signature returns a large error type.
#![allow(clippy::result_large_err)]

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{json, value::Value};
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

const PASSWORD: &str = "long-enough";

#[derive(Default)]
struct Counts {
    connections: AtomicUsize,
    listeners: AtomicUsize,
}

fn devices() -> Value {
    json!([
        {"id": "cam1", "name": "Camera 1", "description": "", "enabled": true, "tslAddress": "1", "linkedBusses": []},
        {"id": "cam2", "name": "Camera 2", "description": "", "enabled": true, "tslAddress": "2", "linkedBusses": []},
    ])
}

fn buses() -> Value {
    json!([
        {"id": "e393251c", "label": "Preview", "type": "preview", "color": "#3fe481", "priority": 50, "visible": true},
        {"id": "334e4eda", "label": "Program", "type": "program", "color": "#e43f5a", "priority": 200, "visible": true},
    ])
}

/// Each device's busses; cam1 on program once `live`.
fn states(live: bool, only: Option<&str>) -> Value {
    let mut out = Vec::new();
    for device in ["cam1", "cam2"] {
        if only.is_some_and(|o| o != device) {
            continue;
        }
        let program = live && device == "cam1";
        out.push(json!({"busId": "e393251c", "deviceId": device, "sources": if device == "cam2" { json!(["ds2"]) } else { json!([]) }}));
        out.push(json!({"busId": "334e4eda", "deviceId": device, "sources": if program { json!(["ds1"]) } else { json!([]) }}));
    }
    Value::Array(out)
}

fn packet(name: &str, args: &[Value]) -> Message {
    let mut items = vec![json!(name)];
    items.extend(args.iter().cloned());
    Message::text(format!("42{}", Value::Array(items)))
}

async fn simulated_tally_arbiter() -> (u16, Arc<Counts>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let counts = Arc::new(Counts::default());
    let shared = counts.clone();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            shared.connections.fetch_add(1, Ordering::SeqCst);
            let counts = shared.clone();
            tokio::spawn(async move {
                let callback = |request: &Request, response: Response| {
                    let uri = request.uri().to_string();
                    assert!(uri.starts_with("/socket.io/?"), "{uri}");
                    assert!(
                        uri.contains("EIO=4") && uri.contains("transport=websocket"),
                        "{uri}"
                    );
                    Ok(response)
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(tcp, callback).await else {
                    return;
                };
                ws.send(Message::text(
                    r#"0{"sid":"engine1","upgrades":[],"pingInterval":25000,"pingTimeout":20000,"maxPayload":1000000}"#,
                ))
                .await
                .unwrap();
                let mut token: Option<String> = None;
                while let Some(Ok(message)) = ws.next().await {
                    let Message::Text(text) = message else {
                        continue;
                    };
                    let text = text.as_str();
                    if text == "40" {
                        ws.send(Message::text(r#"40{"sid":"socket1"}"#))
                            .await
                            .unwrap();
                        ws.send(packet("chat_enabled", &[json!(true)]))
                            .await
                            .unwrap();
                        continue;
                    }
                    let Some(body) = text.strip_prefix("42") else {
                        continue;
                    };
                    let items: Vec<Value> = serde_json::from_str(body).unwrap();
                    let name = items[0].as_str().unwrap().to_string();
                    let args = &items[1..];
                    let admin = token.as_deref() == Some("tok-admin");
                    let mut out = Vec::new();
                    match name.as_str() {
                        "login" => {
                            let ok = args[0] == "admin" && args[1] == PASSWORD;
                            out.push(packet(
                                "login_response",
                                &[if ok {
                                    json!({"loginOk": true, "message": "", "accessToken": "tok-admin"})
                                } else {
                                    json!({"loginOk": false, "message": "Wrong username or password!", "access_token": ""})
                                }],
                            ));
                        }
                        "access_token" => token = args[0].as_str().map(str::to_string),
                        "settings" if admin => out.push(packet(
                            "initialdata",
                            &[
                                json!([]),
                                json!([]),
                                json!({}),
                                json!([]),
                                json!([]),
                                buses(),
                                json!([]),
                                devices(),
                                json!([]),
                                json!([]),
                                json!([]),
                                states(false, None),
                                json!([]),
                                json!([]),
                                json!([]),
                                json!([]),
                            ],
                        )),
                        "settings" | "companion" | "users" | "default_password_users" if !admin => {
                            out.push(packet(
                                "error",
                                &[json!(
                                    "Access token required. Please login to use this feature."
                                )],
                            ))
                        }
                        "users" => out.push(packet(
                            "users",
                            &[json!([{"username": "admin", "roles": "admin"}])],
                        )),
                        "default_password_users" => {
                            out.push(packet("default_password_users", &[json!([])]))
                        }
                        "version" => out.push(packet("version", &[json!("3.3.0")])),
                        "uiVersion" => out.push(packet("uiVersion", &[json!("3.3.0")])),
                        "externalAddress" => out.push(packet(
                            "externalAddress",
                            &[json!("http://127.0.0.1/#/tally")],
                        )),
                        "interfaces" | "sources" | "device_sources" | "device_actions"
                        | "listener_clients" | "tsl_clients" | "cloud_destinations"
                        | "cloud_keys" | "cloud_clients" => out.push(packet(&name, &[json!([])])),
                        "get_error_reports" => out.push(packet("error_reports", &[json!([])])),
                        "bus_options" => out.push(packet("bus_options", &[buses()])),
                        "devices" => out.push(packet("devices", &[devices()])),
                        "device_states" => {
                            let only = args.first().and_then(Value::as_str).map(str::to_string);
                            out.push(packet("device_states", &[states(false, only.as_deref())]));
                        }
                        "listenerclient_connect" => {
                            counts.listeners.fetch_add(1, Ordering::SeqCst);
                            let device = args[0]["deviceId"].as_str().unwrap().to_string();
                            out.push(packet("bus_options", &[buses()]));
                            out.push(packet("devices", &[devices()]));
                            out.push(packet("device_states", &[states(false, Some(&device))]));
                            // The switcher puts the device on program.
                            out.push(packet("device_states", &[states(true, Some(&device))]));
                        }
                        "manage" if admin => {
                            let result =
                                match (args[0]["type"].as_str(), args[0]["action"].as_str()) {
                                    (Some("device"), Some("delete")) => {
                                        "device-deleted-successfully"
                                    }
                                    _ => "error",
                                };
                            out.push(packet("manage_response", &[json!({"result": result})]));
                        }
                        _ => {}
                    }
                    for m in out {
                        if ws.send(m).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
    (port, counts)
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
        device: "tally-arbiter".into(),
        model: "tally-arbiter-3".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"username": "admin", "password": password, "device_id": "cam1"})),
        monitor: true,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn tally_arbiter_end_to_end() {
    let (port, counts) = simulated_tally_arbiter().await;
    let core = common::core().unwrap();
    let id = open(&core, port, PASSWORD);

    // The listener client's device is pushed onto program.
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["devices"]["cam1"]["program"] == true)
    })
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.connection, Connection::Connected);
    assert_eq!(snapshot.state["session"]["access"], "settings");
    assert_eq!(snapshot.state["devices"]["cam1"]["name"], "Camera 1");
    assert_eq!(snapshot.state["devices"]["cam1"]["preview"], false);
    assert_eq!(snapshot.state["devices"]["cam2"]["preview"], true);
    assert_eq!(
        snapshot.state["devices"]["cam2"]["buses"]["334e4eda"],
        false
    );
    assert_eq!(snapshot.state["listener"]["device_id"], "cam1");
    assert_eq!(counts.listeners.load(Ordering::SeqCst), 1);

    let outcome = core.execute(id, "get_version", params(json!({}))).await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!("3.3.0")
        })
    );
    let outcome = core
        .execute(id, "delete_device", params(json!({"device_id": "cam2"})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    let outcome = core
        .execute(id, "add_cloud_key", params(json!({"key": "k"})))
        .await;
    assert!(
        matches!(outcome, Err(CommandError::DeviceError { .. })),
        "{outcome:?}"
    );

    // Reconnecting (a phone resuming) opens a new connection at once and
    // registers the listener client again.
    assert!(core.reconnect(id).await);
    tokio::time::timeout(Duration::from_secs(10), async {
        while counts.listeners.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("listener registered again within 10 s");
    assert_eq!(counts.connections.load(Ordering::SeqCst), 2);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn tally_arbiter_wrong_password_is_unauthorized_and_never_retried() {
    let (port, counts) = simulated_tally_arbiter().await;
    let core = common::core().unwrap();
    let id = open(&core, port, "wrong");
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
    })
    .await;
    // Well past the first reconnect delay.
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(counts.connections.load(Ordering::SeqCst), 1);
    let outcome = core.execute(id, "get_version", params(json!({}))).await;
    assert!(matches!(outcome, Err(CommandError::Auth { .. })));
    core.close(id).await;
}
