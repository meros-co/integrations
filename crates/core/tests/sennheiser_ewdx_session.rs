//! An EW-DX receiver simulated over real TLS with a self-signed certificate,
//! driven through the public API.
//!
//! The simulated device implements the SSCv2 surface the core uses: Basic
//! authentication as "api", /api/ssc/version, /api/device/identity, the
//! subscription stream and its /add endpoint, the channel resources, and mute.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const DEVICE_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);
/// base64("api:secret")
const GOOD_AUTH: &str = "Basic YXBpOnNlY3JldA==";

#[derive(Default)]
struct Device {
    mute: [bool; 2],
    subscribed: Vec<String>,
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

fn channel_resource(device: &Device, id: usize) -> Value {
    json!({"name": format!("Mic {}", id + 1), "mute": device.mute[id]})
}

async fn simulated_ewdx() -> (u16, Arc<Mutex<Device>>) {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(DEVICE_IP), 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = tls_acceptor();
    let device = Arc::new(Mutex::new(Device::default()));
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
                    // Request line and headers.
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
                        let text = body.to_string();
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{text}",
                            text.len()
                        )
                    };

                    if headers.get("authorization").map(String::as_str) != Some(GOOD_AUTH) {
                        let _ = write.write_all(respond(401, json!({})).as_bytes()).await;
                        continue;
                    }

                    let reply = match (method.as_str(), path.as_str()) {
                        ("GET", "/api/ssc/version") => {
                            respond(200, json!({"protocol": "2.0", "schema": "1.5"}))
                        }
                        ("GET", "/api/device/identity") => {
                            respond(200, json!({"product": "EW-DX EM 2", "serial": "123"}))
                        }
                        ("GET", "/api/ssc/state/subscriptions") => {
                            let mut rx = notify.subscribe();
                            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
                            let _ = write.write_all(head.as_bytes()).await;
                            let open = json!({"path": "/api/ssc/state/subscriptions/s1", "sessionUUID": "s1"});
                            let _ = write
                                .write_all(format!("event: open\ndata: {open}\n\n").as_bytes())
                                .await;
                            while let Ok(update) = rx.recv().await {
                                if write
                                    .write_all(format!("data: {update}\n\n").as_bytes())
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
                                    if let Some(id) = p
                                        .strip_prefix("/api/channel/")
                                        .and_then(|s| s.parse::<usize>().ok())
                                    {
                                        initial.insert(p.clone(), channel_resource(&d, id));
                                    }
                                }
                                d.subscribed.extend(paths);
                            }
                            if !initial.is_empty() {
                                let _ = notify.send(Value::Object(initial));
                            }
                            respond(200, json!({}))
                        }
                        ("GET", p) if p.starts_with("/api/channel/") && !p[13..].contains('/') => {
                            let id: usize = p[13..].parse().unwrap();
                            if id < 2 {
                                respond(200, channel_resource(&device.lock().unwrap(), id))
                            } else {
                                respond(404, json!({}))
                            }
                        }
                        ("GET", p) if p.starts_with("/api/rf/channels/") => {
                            respond(200, json!({"frequency": 606500}))
                        }
                        ("GET", p) if p.starts_with("/api/transmitters/") => {
                            // Channel 2 has no transmitter linked.
                            if p.contains("/1/") {
                                respond(422, json!({}))
                            } else {
                                respond(200, json!({"gauge": 65}))
                            }
                        }
                        ("GET", p) if p.ends_with("/signalQualityIndicator") => {
                            respond(200, json!({"value": 90}))
                        }
                        ("GET", p) if p.ends_with("/level") => {
                            respond(200, json!({"value": -40.0}))
                        }
                        ("PUT", p) if p.starts_with("/api/channel/") => {
                            let id: usize = p[13..].parse().unwrap();
                            let muted = body["mute"].as_bool().unwrap();
                            device.lock().unwrap().mute[id] = muted;
                            let _ = notify.send(json!({ p: {"mute": muted} }));
                            respond(200, json!({}))
                        }
                        _ => respond(404, json!({})),
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
        device: "sennheiser-ew-dx".into(),
        model: "em-2".into(),
        host: DEVICE_IP.to_string(),
        port: Some(port),
        settings: params(json!({ "password": password })),
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn ewdx_end_to_end() {
    let (port, device) = simulated_ewdx().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "secret");

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["identity"]["product"], "EW-DX EM 2");
    assert_eq!(state["channels"]["1"]["name"], "Mic 1");
    assert_eq!(state["channels"]["1"]["frequency_khz"], 606500);
    assert_eq!(state["channels"]["1"]["transmitter"]["battery_percent"], 65);
    assert_eq!(state["channels"]["2"]["transmitter"], Value::Null);
    // Six resources per channel, both channels.
    assert_eq!(device.lock().unwrap().subscribed.len(), 12);

    let outcome = core
        .execute(id, "mute", params(json!({"channel": 2, "muted": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(
        &core,
        |e| matches!(e, Event::State { patch, .. } if patch["channels"]["2"]["mute"] == true),
    )
    .await;
    assert!(device.lock().unwrap().mute[1]);

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ewdx_wrong_password_is_unauthorized() {
    let (port, _device) = simulated_ewdx().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "wrong");

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
    })
    .await;
    let outcome = core
        .execute(id, "mute", params(json!({"channel": 1})))
        .await;
    assert_eq!(outcome, Err(CommandError::NotConnected));
}
