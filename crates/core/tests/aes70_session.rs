//! An AES70 device simulated on real loopback sockets, driven through the
//! public API, over each OCP.1 transport: TCP, UDP (one PDU per datagram)
//! and WebSocket binary messages, plain and over TLS.
//!
//! The simulator speaks OCP.1 as the open implementations describe it (see
//! specs/aes70.yaml): PDUs of a sync byte, a 9-byte header and several
//! messages, responses matched by handle, keep-alives echoed, and
//! PropertyChanged notifications for objects subscribed to with
//! AddSubscription. Its tree: the root block (100) holds "In1" (a block,
//! 200) and "Out" (an OcaGain, 300); "In1" holds "Gain" (an OcaGain, 201)
//! and "Mute" (an OcaMute, 202). Gains are clamped to their maximum of 12 dB
//! and the clamped value notified, as a device may do.
//!
//! The encoder and decoder here are written apart from the module's, from
//! the same description, so the two check each other.

#![cfg(feature = "aes70")]

mod common;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meros_integrations::{json, value::Value};
use meros_integrations::{
    CommandError, Connection, Core, DeviceId, Event, OpenError, OpenRequest, Outcome,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

const GAIN: &[u16] = &[1, 1, 1, 5];
const MUTE: &[u16] = &[1, 1, 1, 2];
const BLOCK: &[u16] = &[1, 1, 3];

/// What the device was asked, as (target, method level, method index).
type Log = Arc<Mutex<Vec<(u32, u16, u16)>>>;

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// A PDU: sync, version 1, size after the sync byte, type, count, messages.
fn pdu(kind: u8, messages: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = messages.concat();
    let mut out = vec![0x3B];
    out.extend(be16(1));
    out.extend(be32(9 + body.len() as u32));
    out.push(kind);
    out.extend(be16(messages.len() as u16));
    out.extend(body);
    out
}

/// A response message: size (itself included), handle, status, count, params.
fn response(handle: u32, status: u8, count: u8, params: &[u8]) -> Vec<u8> {
    let mut m = be32(10 + params.len() as u32).to_vec();
    m.extend(be32(handle));
    m.push(status);
    m.push(count);
    m.extend(params);
    m
}

/// A PropertyChanged notification: target and method of the subscriber,
/// two parameters (an empty context and the event), the emitter, event 1.1,
/// and the data: property id, value, change type.
fn property_changed(emitter: u32, prop: (u16, u16), value: &[u8], change: u8) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(be32(4096));
    body.extend(be16(1));
    body.extend(be16(1));
    body.push(2);
    body.extend(be16(0));
    body.extend(be32(emitter));
    body.extend(be16(1));
    body.extend(be16(1));
    body.extend(be16(prop.0));
    body.extend(be16(prop.1));
    body.extend(value);
    body.push(change);
    let mut m = be32(4 + body.len() as u32).to_vec();
    m.extend(body);
    m
}

fn string(s: &str) -> Vec<u8> {
    let mut out = be16(s.chars().count() as u16).to_vec();
    out.extend(s.as_bytes());
    out
}

fn ident(ono: u32, class: &[u16]) -> Vec<u8> {
    let mut out = be32(ono).to_vec();
    out.extend(be16(class.len() as u16));
    for f in class {
        out.extend(be16(*f));
    }
    out.extend(be16(1));
    out
}

struct Device {
    gains: [(u32, f32); 2],
    muted: bool,
    subscribed: BTreeSet<u32>,
}

impl Device {
    fn gain(&mut self, ono: u32) -> Option<&mut f32> {
        self.gains
            .iter_mut()
            .find(|(o, _)| *o == ono)
            .map(|(_, g)| g)
    }

    /// The response to a command, and any notification it causes.
    fn handle(
        &mut self,
        target: u32,
        method: (u16, u16),
        params: &[u8],
    ) -> ((u8, u8, Vec<u8>), Option<Vec<u8>>) {
        let ok = |count: u8, p: Vec<u8>| (0u8, count, p);
        match (target, method) {
            (100, (3, 5)) => {
                let mut p = be16(2).to_vec();
                p.extend(ident(200, BLOCK));
                p.extend(ident(300, GAIN));
                (ok(1, p), None)
            }
            (200, (3, 5)) => {
                let mut p = be16(2).to_vec();
                p.extend(ident(201, GAIN));
                p.extend(ident(202, MUTE));
                (ok(1, p), None)
            }
            (_, (1, 5)) => {
                let role = match target {
                    200 => "In1",
                    300 => "Out",
                    201 => "Gain",
                    202 => "Mute",
                    _ => return ((5, 0, vec![]), None),
                };
                (ok(1, string(role)), None)
            }
            (1, (3, 6)) => {
                let mut p = string("Simulated");
                p.extend(string("Stagebox"));
                p.extend(string("1.0"));
                (ok(1, p), None)
            }
            (1, (3, 3)) => (ok(1, string("SIM-0001")), None),
            (1, _) => ((8, 0, vec![]), None),
            (4, (3, 1)) => {
                // AddSubscription: the event's emitter comes first.
                let emitter = u32::from_be_bytes(params[..4].try_into().unwrap());
                self.subscribed.insert(emitter);
                (ok(0, vec![]), None)
            }
            (201 | 300, (4, 1)) => {
                let g = *self.gain(target).unwrap();
                let mut p = g.to_be_bytes().to_vec();
                p.extend((-120f32).to_be_bytes());
                p.extend(12f32.to_be_bytes());
                (ok(3, p), None)
            }
            (201 | 300, (4, 2)) => {
                let wanted = f32::from_be_bytes(params[..4].try_into().unwrap());
                let clamped = wanted.min(12.0);
                *self.gain(target).unwrap() = clamped;
                let note = self
                    .subscribed
                    .contains(&target)
                    .then(|| property_changed(target, (4, 1), &clamped.to_be_bytes(), 1));
                (ok(0, vec![]), note)
            }
            (202, (4, 1)) => (ok(1, vec![if self.muted { 1 } else { 2 }]), None),
            (202, (4, 2)) => {
                self.muted = params == [1];
                let note = self
                    .subscribed
                    .contains(&202)
                    .then(|| property_changed(202, (4, 1), params, 1));
                (ok(0, vec![]), note)
            }
            _ => ((11, 0, vec![]), None),
        }
    }
}

/// The device behind every transport: answers each complete PDU with the
/// PDUs it sends back, until `silent` is set, after which it says nothing.
struct Sim {
    device: Device,
    log: Log,
    silent: Arc<AtomicBool>,
}

impl Sim {
    fn new(silent: Arc<AtomicBool>) -> (Sim, Log) {
        let log: Log = Arc::default();
        let sim = Sim {
            device: Device {
                gains: [(201, -6.0), (300, 0.0)],
                muted: false,
                subscribed: BTreeSet::new(),
            },
            log: log.clone(),
            silent,
        };
        (sim, log)
    }

    fn answer(&mut self, p: &[u8]) -> Vec<Vec<u8>> {
        assert_eq!(p[0], 0x3B, "sync byte");
        if self.silent.load(Ordering::SeqCst) {
            return vec![];
        }
        let kind = p[7];
        let count = u16::from_be_bytes([p[8], p[9]]);
        if kind == 4 {
            // Echo the controller's keep-alive.
            return vec![p.to_vec()];
        }
        assert_eq!(kind, 1, "commands want a response");
        let mut at = 10;
        let mut replies = Vec::new();
        let mut notes = Vec::new();
        for _ in 0..count {
            let m = &p[at..];
            let msize = u32::from_be_bytes(m[..4].try_into().unwrap()) as usize;
            let handle = u32::from_be_bytes(m[4..8].try_into().unwrap());
            let target = u32::from_be_bytes(m[8..12].try_into().unwrap());
            let level = u16::from_be_bytes([m[12], m[13]]);
            let index = u16::from_be_bytes([m[14], m[15]]);
            let params = &m[17..msize];
            self.log.lock().unwrap().push((target, level, index));
            let ((status, pc, rp), note) = self.device.handle(target, (level, index), params);
            replies.push(response(handle, status, pc, &rp));
            notes.extend(note);
            at += msize;
        }
        let mut out = vec![pdu(3, &replies)];
        if !notes.is_empty() {
            out.push(pdu(2, &notes));
        }
        out
    }

    /// Complete PDUs taken from the front of a byte stream.
    fn take_pdus(pending: &mut Vec<u8>) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while pending.len() >= 10 {
            assert_eq!(pending[0], 0x3B, "sync byte");
            let size = u32::from_be_bytes(pending[3..7].try_into().unwrap()) as usize;
            if pending.len() < size + 1 {
                break;
            }
            out.push(pending.drain(..size + 1).collect());
        }
        out
    }

    /// Serve a byte stream until it closes.
    async fn stream(mut self, mut stream: impl AsyncRead + AsyncWrite + Unpin) {
        let mut buf = vec![0u8; 65536];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            for p in Sim::take_pdus(&mut pending) {
                for out in self.answer(&p) {
                    stream.write_all(&out).await.unwrap();
                }
            }
        }
    }

    /// Serve WebSocket binary messages, a byte stream across messages
    /// (AES70-3 8.4.3.4.4); the controller must offer AES70-OCP.1.
    async fn websocket(mut self, stream: impl AsyncRead + AsyncWrite + Unpin) {
        let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, offers_ocp1).await else {
            return;
        };
        let mut pending: Vec<u8> = Vec::new();
        while let Some(Ok(message)) = ws.next().await {
            let Message::Binary(data) = message else {
                assert!(!message.is_text(), "OCP.1 is never sent as text");
                continue;
            };
            pending.extend_from_slice(&data);
            for p in Sim::take_pdus(&mut pending) {
                for out in self.answer(&p) {
                    if ws.send(Message::binary(out)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

/// The WebSocket handshake: the controller must offer AES70-OCP.1, which is
/// accepted.
#[allow(clippy::result_large_err)]
fn offers_ocp1(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
    let offered = request
        .headers()
        .get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(offered, "AES70-OCP.1", "the OCP.1 subprotocol is offered");
    response.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("AES70-OCP.1"),
    );
    Ok(response)
}

/// Accept one TCP connection and serve it until `silent` is set, after
/// which the device reads but says nothing.
async fn simulate(silent: Arc<AtomicBool>) -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sim, log) = Sim::new(silent);
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        sim.stream(stream).await;
    });
    (port, log)
}

/// Serve OCP.1 over UDP: every datagram must be one whole PDU, and the
/// answers go back to its sender, one PDU per datagram.
async fn simulate_udp(silent: Arc<AtomicBool>) -> (u16, Log, Arc<Mutex<usize>>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let (mut sim, log) = Sim::new(silent);
    let largest: Arc<Mutex<usize>> = Arc::default();
    let seen = largest.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let mut opened = false;
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let datagram = buf[..n].to_vec();
            {
                let mut l = seen.lock().unwrap();
                *l = (*l).max(n);
            }
            let size = u32::from_be_bytes(datagram[3..7].try_into().unwrap()) as usize;
            assert_eq!(size + 1, n, "one whole PDU per datagram");
            // A UDP device ignores everything before the controller's
            // keep-alive.
            if datagram[7] == 4 {
                opened = true;
            }
            assert!(opened, "the keep-alive comes first");
            for out in sim.answer(&datagram) {
                socket.send_to(&out, from).await.unwrap();
            }
        }
    });
    (port, log, largest)
}

/// Serve OCP.1 over a WebSocket, over TLS with a self-signed certificate
/// when `secure`.
async fn simulate_ws(silent: Arc<AtomicBool>, secure: bool) -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sim, log) = Sim::new(silent);
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        if secure {
            let Ok(tls) = tls_acceptor().accept(tcp).await else {
                return;
            };
            sim.websocket(tls).await;
        } else {
            sim.websocket(tcp).await;
        }
    });
    (port, log)
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

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn wait_for_connection(core: &Core, id: DeviceId, pred: impl Fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            for e in core.next_events(64).await {
                if let Event::Connection { device, connection } = e {
                    if device == id && pred(&connection) {
                        return;
                    }
                }
            }
        }
    })
    .await
    .expect("connection state within 10 s")
}

async fn wait_for_state(core: &Core, id: DeviceId, pred: impl Fn(&Value) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn ocp1_end_to_end() {
    let silent = Arc::new(AtomicBool::new(false));
    let (port, log) = simulate(silent.clone()).await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "aes70".into(),
            model: "aes70-device".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"keepalive_interval_ms": 200})),
            monitor: true,
        })
        .unwrap();

    // Connected, the tree walked, the known objects read.
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    wait_for_state(&core, id, |s| {
        s["discovery"]["complete"] == true
            && s["objects"]["In1/Gain"]["gain_db"] == -6.0
            && s["objects"]["In1/Mute"]["muted"] == false
            && s["objects"]["Out"]["gain_db_max"] == 12.0
            && s["device"]["serial_number"] == "SIM-0001"
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["objects"]["In1"]["class"], "OcaBlock");
    assert_eq!(state["objects"]["In1/Gain"]["ono"], 201);
    assert_eq!(state["device"]["model"]["name"], "Stagebox");

    // Read a gain.
    assert_eq!(
        core.execute(
            id,
            "get_property",
            params(json!({"object": "In1/Gain", "property": "4.1"}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"value": -6.0, "min": -120.0, "max": 12.0})
        })
    );

    // Set a gain: acknowledged, and the device's clamped value arrives by
    // notification.
    assert_eq!(
        core.execute(
            id,
            "set_gain",
            params(json!({"object": "In1/Gain", "gain_db": 20.0}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["In1/Gain"]["gain_db"] == 12.0).await;
    // By object number.
    assert_eq!(
        core.execute(id, "set_mute", params(json!({"ono": 202, "muted": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["In1/Mute"]["muted"] == true).await;
    // Refused before sending: the object is not a gain.
    assert!(matches!(
        core.execute(
            id,
            "set_gain",
            params(json!({"object": "In1/Mute", "gain_db": 0.0}))
        )
        .await,
        Err(CommandError::InvalidParams { .. })
    ));
    // A status other than OK.
    assert_eq!(
        core.execute(
            id,
            "set_property",
            params(json!({"object": "Out", "property": "2.3", "value": "Main"}))
        )
        .await,
        Err(CommandError::DeviceError {
            code: Some("11".into()),
            message: "BadMethod: no such method".into()
        })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());

    // Subscriptions to the device manager and the three known objects.
    let asked = log.lock().unwrap().clone();
    let subs = asked.iter().filter(|a| **a == (4, 3, 1)).count();
    assert_eq!(subs, 4);
    assert!(asked.contains(&(100, 3, 5)) && asked.contains(&(200, 3, 5)));

    // The device goes quiet: three keep-alive intervals later it is lost.
    silent.store(true, Ordering::SeqCst);
    wait_for_connection(&core, id, |c| matches!(c, Connection::Disconnected { .. })).await;
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_only_walks_nothing_and_looks_up_what_a_command_names() {
    let (port, log) = simulate(Arc::new(AtomicBool::new(false))).await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "aes70".into(),
            model: "aes70-device".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"keepalive_interval_ms": 200})),
            monitor: false,
        })
        .unwrap();
    // The echoed keep-alive makes it connected.
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    assert!(
        log.lock().unwrap().is_empty(),
        "no walk, read or subscription"
    );

    assert_eq!(
        core.execute(
            id,
            "set_gain",
            params(json!({"object": "In1/Gain", "gain_db": -3.0}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    let asked = log.lock().unwrap().clone();
    assert!(!asked.iter().any(|a| a.0 == 4), "no subscription");
    assert!(!asked.iter().any(|a| (a.1, a.2) == (4, 1)), "no reads");
    assert_eq!(asked.last(), Some(&(201, 4, 2)));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_port_is_required() {
    let core = common::core().unwrap();
    let opened = core.open(OpenRequest {
        device: "aes70".into(),
        model: "aes70-device".into(),
        host: "127.0.0.1".into(),
        port: None,
        settings: params(json!({})),
        monitor: true,
    });
    match opened {
        Err(OpenError::NotImplemented { reason, .. }) => {
            assert!(reason.contains("port"), "{reason}")
        }
        other => panic!("{other:?}"),
    }
}

/// Open over `transport`, walk, set a gain and see the notification, then
/// lose the device when it goes quiet.
async fn end_to_end_over(transport: &str, port: u16, log: &Log, silent: &AtomicBool) {
    let core = common::core().unwrap();
    let mut settings = json!({"keepalive_interval_ms": 200, "transport": transport});
    if transport == "wss" {
        settings["accept_invalid_certs"] = json!(true);
    }
    let id = core
        .open(OpenRequest {
            device: "aes70".into(),
            model: "aes70-device".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(settings),
            monitor: true,
        })
        .unwrap();
    wait_for_connection(&core, id, |c| *c == Connection::Connected).await;
    wait_for_state(&core, id, |s| {
        s["discovery"]["complete"] == true
            && s["objects"]["In1/Gain"]["gain_db"] == -6.0
            && s["objects"]["In1/Mute"]["muted"] == false
            && s["device"]["serial_number"] == "SIM-0001"
    })
    .await;
    assert_eq!(
        core.execute(
            id,
            "set_gain",
            params(json!({"object": "In1/Gain", "gain_db": 20.0}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["In1/Gain"]["gain_db"] == 12.0).await;
    assert_eq!(
        core.execute(id, "set_mute", params(json!({"ono": 202, "muted": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["In1/Mute"]["muted"] == true).await;
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    let subs = log
        .lock()
        .unwrap()
        .iter()
        .filter(|a| **a == (4, 3, 1))
        .count();
    assert_eq!(subs, 4);
    silent.store(true, Ordering::SeqCst);
    wait_for_connection(&core, id, |c| matches!(c, Connection::Disconnected { .. })).await;
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ocp1_over_udp() {
    let silent = Arc::new(AtomicBool::new(false));
    let (port, log, largest) = simulate_udp(silent.clone()).await;
    end_to_end_over("udp", port, &log, &silent).await;
    assert!(*largest.lock().unwrap() <= 1452, "datagrams fit one frame");
}

#[tokio::test(flavor = "multi_thread")]
async fn ocp1_over_websocket() {
    let silent = Arc::new(AtomicBool::new(false));
    let (port, log) = simulate_ws(silent.clone(), false).await;
    end_to_end_over("ws", port, &log, &silent).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ocp1_over_secure_websocket() {
    let silent = Arc::new(AtomicBool::new(false));
    let (port, log) = simulate_ws(silent.clone(), true).await;
    end_to_end_over("wss", port, &log, &silent).await;
}
