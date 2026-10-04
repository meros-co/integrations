//! A Spectera Base Station simulated over real TLS with a self-signed
//! certificate, driven through the public API.
//!
//! The simulated device implements the SSCv2 surface the core uses: Basic
//! authentication as controlSennheiser, /api/ssc/version, the subscription
//! stream and its /add endpoint (sending each added resource's current value,
//! collections as whole lists), RF channel writes notified as the changed
//! item on the collection's path, audio link creation notified on the item's
//! path, and mobile device writes that must name the device's type.

#![cfg(feature = "sennheiser-spectera")]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const DEVICE_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 3);
/// base64("controlSennheiser:secret")
const GOOD_AUTH: &str = "Basic Y29udHJvbFNlbm5oZWlzZXI6c2VjcmV0";

struct Device {
    rf_state: [&'static str; 2],
    subscribed: Vec<String>,
    /// The last mobile device write's body.
    mobile_write: Value,
    refused: usize,
}

fn tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
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

fn current(device: &Device, path: &str) -> Option<Value> {
    Some(match path {
        "/api/device/identity" => {
            json!({"product": "Spectera-Base-Station", "serial": "1234567890"})
        }
        "/api/device/state" => json!({"state": "Normal", "warnings": []}),
        "/api/rf/channels" => json!([
            {"rfChannelId": 0, "frequency": 550000, "rfState": device.rf_state[0]},
            {"rfChannelId": 1, "frequency": 600000, "rfState": device.rf_state[1]}
        ]),
        "/api/mts/paired/all" => json!([
            {"mtUid": 77, "type": "SEK", "name": "Lead", "batteryFillLevel": 90, "micLqi": 4}
        ]),
        "/api/audio/links" => json!([]),
        _ => return None,
    })
}

async fn simulated_spectera() -> (u16, Arc<Mutex<Device>>) {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(DEVICE_IP), 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = tls_acceptor();
    let device = Arc::new(Mutex::new(Device {
        rf_state: ["RfActive", "RfActive"],
        subscribed: Vec::new(),
        mobile_write: Value::Null,
        refused: 0,
    }));
    let (notify, _) = broadcast::channel::<Value>(64);

    let state = device.clone();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            let device = state.clone();
            let notify = notify.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let (read, mut write) = tokio::io::split(tls);
                let mut reader = BufReader::new(read);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let (method, path) = (
                        parts.next().unwrap_or("").to_string(),
                        parts.next().unwrap_or("").to_string(),
                    );
                    let mut headers = HashMap::new();
                    loop {
                        let mut h = String::new();
                        reader.read_line(&mut h).await.unwrap();
                        let h = h.trim_end();
                        if h.is_empty() {
                            break;
                        }
                        if let Some((k, v)) = h.split_once(':') {
                            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
                        }
                    }
                    let len: usize = headers
                        .get("content-length")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let mut body = vec![0u8; len];
                    reader.read_exact(&mut body).await.unwrap();
                    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

                    let respond = |status: u16, body: Value| {
                        let text = if body.is_null() {
                            String::new()
                        } else {
                            body.to_string()
                        };
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{text}",
                            text.len()
                        )
                    };

                    // The version and identity need no authentication.
                    let open_path = path == "/api/ssc/version" || path == "/api/device/identity";
                    if !open_path
                        && headers.get("authorization").map(String::as_str) != Some(GOOD_AUTH)
                    {
                        device.lock().unwrap().refused += 1;
                        let _ = write.write_all(respond(401, Value::Null).as_bytes()).await;
                        continue;
                    }

                    let reply = match (method.as_str(), path.as_str()) {
                        ("GET", "/api/ssc/version") => {
                            respond(200, json!({"protocol": "2.3", "schema": "18.1"}))
                        }
                        ("GET", "/api/ssc/state/subscriptions") => {
                            let mut rx = notify.subscribe();
                            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Location: /api/ssc/state/subscriptions/s1\r\nConnection: close\r\n\r\n";
                            let _ = write.write_all(head.as_bytes()).await;
                            let open = json!({"path": "/api/ssc/state/subscriptions/s1", "sessionUUID": "s1"});
                            let _ = write
                                .write_all(format!("event: open\ndata: {open}\n\n").as_bytes())
                                .await;
                            while let Ok(update) = rx.recv().await {
                                if write
                                    .write_all(
                                        format!("event: message\ndata: {update}\n\n").as_bytes(),
                                    )
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            return;
                        }
                        ("PUT", "/api/ssc/state/subscriptions/s1/add") => {
                            let paths: Vec<String> =
                                serde_json::from_value(body).unwrap_or_default();
                            let mut initial = serde_json::Map::new();
                            {
                                let mut d = device.lock().unwrap();
                                for p in &paths {
                                    if let Some(v) = current(&d, p) {
                                        initial.insert(p.clone(), v);
                                    }
                                }
                                d.subscribed.extend(paths);
                            }
                            if !initial.is_empty() {
                                let _ = notify.send(Value::Object(initial));
                            }
                            respond(200, Value::Null)
                        }
                        ("GET", "/api/device/identity") => {
                            respond(200, current(&device.lock().unwrap(), &path).unwrap())
                        }
                        ("PUT", p) if p.starts_with("/api/rf/channels/") => {
                            let id: usize = p[17..].parse().unwrap();
                            let state = if body["rfState"] == "RfMuted" {
                                "RfMuted"
                            } else {
                                "RfActive"
                            };
                            device.lock().unwrap().rf_state[id] = state;
                            let _ = notify.send(
                                json!({"/api/rf/channels": {"rfChannelId": id, "rfState": state}}),
                            );
                            respond(200, Value::Null)
                        }
                        ("POST", "/api/audio/links") => {
                            let link = json!({"audiolinkId": 3, "rfChannelId": body["rfChannelId"], "modeId": body["modeId"]});
                            let _ = notify.send(json!({"/api/audio/links/3": {}}));
                            let _ = notify.send(json!({"/api/audio/links/3": link.clone()}));
                            respond(201, link)
                        }
                        ("PUT", p) if p.starts_with("/api/mts/paired/all/") => {
                            if body.get("type").is_none() {
                                respond(400, json!({"path": p, "error": 404}))
                            } else {
                                device.lock().unwrap().mobile_write = body.clone();
                                let _ = notify.send(json!({ p: {"mtUid": 77, "micAudiolinkId": body["micAudiolinkId"]} }));
                                respond(200, Value::Null)
                            }
                        }
                        _ => respond(422, Value::Null),
                    };
                    if write.write_all(reply.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, device)
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
        device: "sennheiser-spectera".into(),
        model: "base-station".into(),
        host: DEVICE_IP.to_string(),
        port: Some(port),
        settings: params(json!({ "password": password })),
        monitor: true,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn spectera_end_to_end() {
    let (port, device) = simulated_spectera().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "secret");

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(800)).await;

    let state = core.snapshot(id).unwrap().state;
    assert_eq!(
        state["device"]["identity"]["product"],
        "Spectera-Base-Station"
    );
    assert_eq!(state["device"]["status"]["state"], "Normal");
    assert_eq!(state["rf"]["channels"]["1"]["frequency"], 600000);
    assert_eq!(state["mobile_devices"]["77"]["battery_fill_level"], 90);
    assert!(device
        .lock()
        .unwrap()
        .subscribed
        .contains(&"/api/mts/paired/all".to_string()));

    let outcome = core
        .execute(
            id,
            "set_rf_active",
            params(json!({"rf_channel": 1, "active": false})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(&core, |e| {
        matches!(e, Event::State { patch, .. } if patch["rf"]["channels"]["1"]["rf_state"] == "RfMuted")
    })
    .await;
    // The other channel's state is untouched by a single-item notification.
    assert_eq!(
        core.snapshot(id).unwrap().state["rf"]["channels"]["0"]["frequency"],
        550000
    );

    let outcome = core
        .execute(
            id,
            "create_audio_link",
            params(json!({"rf_channel": 0, "mode": "live_mono"})),
        )
        .await;
    assert!(matches!(outcome, Ok(Outcome::Value { ref value }) if value["audiolinkId"] == 3));

    // The type comes from the paired list in state.
    let outcome = core
        .execute(
            id,
            "assign_mic_link",
            params(json!({"mt_uid": 77, "link": 3})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    assert_eq!(
        device.lock().unwrap().mobile_write,
        json!({"mtUid": 77, "type": "SEK", "micAudiolinkId": 3})
    );
    wait_for(&core, |e| {
        matches!(e, Event::State { patch, .. } if patch["mobile_devices"]["77"]["mic_audiolink_id"] == 3)
    })
    .await;
    assert_eq!(
        core.snapshot(id).unwrap().state["audio"]["links"]["3"]["mode_id"],
        4
    );

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn spectera_wrong_password_is_unauthorized_and_never_retried() {
    let (port, device) = simulated_spectera().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "wrong");

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
    })
    .await;
    let outcome = core.execute(id, "identify", params(json!({}))).await;
    assert!(matches!(outcome, Err(CommandError::Auth { .. })));

    tokio::time::sleep(Duration::from_millis(2_500)).await;
    // The identity read needs no password; the stream is the one refused.
    assert_eq!(device.lock().unwrap().refused, 1);
    core.close(id).await;
}
