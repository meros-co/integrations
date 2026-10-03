//! An AES70 device simulated on real TCP, driven through the public API.
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

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{
    CommandError, Connection, Core, DeviceId, Event, OpenError, OpenRequest, Outcome,
};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

/// Accept one connection and serve it until `silent` is set, after which
/// the device reads but says nothing.
async fn simulate(silent: Arc<AtomicBool>) -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut device = Device {
            gains: [(201, -6.0), (300, 0.0)],
            muted: false,
            subscribed: BTreeSet::new(),
        };
        let mut buf = vec![0u8; 65536];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            let mut out = Vec::new();
            while pending.len() >= 10 {
                assert_eq!(pending[0], 0x3B, "sync byte");
                let size = u32::from_be_bytes(pending[3..7].try_into().unwrap()) as usize;
                if pending.len() < size + 1 {
                    break;
                }
                let p: Vec<u8> = pending.drain(..size + 1).collect();
                if silent.load(Ordering::SeqCst) {
                    continue;
                }
                let kind = p[7];
                let count = u16::from_be_bytes([p[8], p[9]]);
                if kind == 4 {
                    // Echo the controller's keep-alive.
                    out.push(p.clone());
                    continue;
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
                    record.lock().unwrap().push((target, level, index));
                    let ((status, pc, rp), note) = device.handle(target, (level, index), params);
                    replies.push(response(handle, status, pc, &rp));
                    notes.extend(note);
                    at += msize;
                }
                out.push(pdu(3, &replies));
                if !notes.is_empty() {
                    out.push(pdu(2, &notes));
                }
            }
            for p in out {
                stream.write_all(&p).await.unwrap();
            }
        }
    });
    (port, log)
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
    let core = Core::new().unwrap();
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
    let core = Core::new().unwrap();
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
    let core = Core::new().unwrap();
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
