//! A d&b D80 simulated on loopback TCP, driven through the public API: its
//! objects numbered as d&b's Q-SYS plug-in numbers them (type << 28 |
//! record << 20 | channel << 15 | object), answered over OCP.1 as the aes70
//! session test's device is. It knows the power switch, channel A's mute and
//! level, and the AmpPreset agent; it answers BadONo for anything else.

#![cfg(feature = "dbaudio")]

mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, DeviceId, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const POWER: u32 = 0x1000_0100;
const MUTE_A: u32 = 0x1000_8205;
const LEVEL_A: u32 = 0x1000_8206;
const PRESETS: u32 = 10003;

/// What the device was asked, as (target, method level, method index, params).
type Log = Arc<Mutex<Vec<(u32, u16, u16, Vec<u8>)>>>;

fn pdu(kind: u8, messages: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = messages.concat();
    let mut out = vec![0x3B, 0, 1];
    out.extend((9 + body.len() as u32).to_be_bytes());
    out.push(kind);
    out.extend((messages.len() as u16).to_be_bytes());
    out.extend(body);
    out
}

fn response(handle: u32, status: u8, count: u8, params: &[u8]) -> Vec<u8> {
    let mut m = (10 + params.len() as u32).to_be_bytes().to_vec();
    m.extend(handle.to_be_bytes());
    m.push(status);
    m.push(count);
    m.extend(params);
    m
}

fn property_changed(emitter: u32, value: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(4096u32.to_be_bytes());
    body.extend([0, 1, 0, 1, 2, 0, 0]);
    body.extend(emitter.to_be_bytes());
    body.extend([0, 1, 0, 1, 0, 4, 0, 1]);
    body.extend(value);
    body.push(1);
    let mut m = (4 + body.len() as u32).to_be_bytes().to_vec();
    m.extend(body);
    m
}

fn class(fields: &[u16]) -> Vec<u8> {
    let mut out = (fields.len() as u16).to_be_bytes().to_vec();
    for f in fields {
        out.extend(f.to_be_bytes());
    }
    out.extend(1u16.to_be_bytes());
    out
}

struct Amp {
    powered: bool,
    muted: bool,
    level: f32,
    subscribed: BTreeSet<u32>,
}

impl Amp {
    fn handle(
        &mut self,
        target: u32,
        method: (u16, u16),
        params: &[u8],
    ) -> ((u8, u8, Vec<u8>), Option<Vec<u8>>) {
        let ok = |count: u8, p: Vec<u8>| (0u8, count, p);
        let note = |amp: &Amp, ono: u32, value: &[u8]| {
            amp.subscribed
                .contains(&ono)
                .then(|| property_changed(ono, value))
        };
        match (target, method) {
            (POWER, (1, 1)) => (ok(1, class(&[1, 1, 1, 4])), None),
            (MUTE_A, (1, 1)) => (ok(1, class(&[1, 1, 1, 2])), None),
            (LEVEL_A, (1, 1)) => (ok(1, class(&[1, 1, 1, 5])), None),
            (POWER, (4, 1)) => {
                let mut p = (self.powered as u16).to_be_bytes().to_vec();
                p.extend(0u16.to_be_bytes());
                p.extend(1u16.to_be_bytes());
                (ok(3, p), None)
            }
            (POWER, (4, 2)) => {
                self.powered = params == [0, 1];
                (ok(0, vec![]), note(self, POWER, params))
            }
            (MUTE_A, (4, 1)) => (ok(1, vec![if self.muted { 1 } else { 2 }]), None),
            (MUTE_A, (4, 2)) => {
                self.muted = params == [1];
                (ok(0, vec![]), note(self, MUTE_A, params))
            }
            (LEVEL_A, (4, 1)) => {
                let mut p = self.level.to_be_bytes().to_vec();
                p.extend((-57.5f32).to_be_bytes());
                p.extend(6f32.to_be_bytes());
                (ok(3, p), None)
            }
            (LEVEL_A, (4, 2)) => {
                self.level = f32::from_be_bytes(params[..4].try_into().unwrap());
                (ok(0, vec![]), note(self, LEVEL_A, params))
            }
            (PRESETS, (3, 3)) => (ok(0, vec![]), None),
            (4, (3, 1)) => {
                let emitter = u32::from_be_bytes(params[..4].try_into().unwrap());
                self.subscribed.insert(emitter);
                (ok(0, vec![]), None)
            }
            (1, _) => ((8, 0, vec![]), None),
            _ => ((5, 0, vec![]), None),
        }
    }
}

async fn simulate() -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut amp = Amp {
            powered: true,
            muted: false,
            level: -10.0,
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
            while pending.len() >= 10 {
                let size = u32::from_be_bytes(pending[3..7].try_into().unwrap()) as usize;
                if pending.len() < size + 1 {
                    break;
                }
                let p: Vec<u8> = pending.drain(..size + 1).collect();
                if p[7] == 4 {
                    stream.write_all(&p).await.unwrap();
                    continue;
                }
                let count = u16::from_be_bytes([p[8], p[9]]);
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
                    let params = m[17..msize].to_vec();
                    record
                        .lock()
                        .unwrap()
                        .push((target, level, index, params.clone()));
                    let ((status, pc, rp), note) = amp.handle(target, (level, index), &params);
                    replies.push(response(handle, status, pc, &rp));
                    notes.extend(note);
                    at += msize;
                }
                stream.write_all(&pdu(3, &replies)).await.unwrap();
                if !notes.is_empty() {
                    stream.write_all(&pdu(2, &notes)).await.unwrap();
                }
            }
        }
    });
    (port, log)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
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
async fn a_d80_by_its_named_objects() {
    let (port, log) = simulate().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "dbaudio".into(),
            model: "d80".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"keepalive_interval_ms": 200, "walk": false})),
            monitor: true,
        })
        .unwrap();

    // The named objects the device knows are read and followed by name.
    wait_for_state(&core, id, |s| {
        s["objects"]["Power"]["position"] == 1
            && s["objects"]["A/Mute"]["muted"] == false
            && s["objects"]["A/Level"]["gain_db"] == -10.0
    })
    .await;
    assert_eq!(
        core.snapshot(id).unwrap().state["objects"]["A/Mute"]["ono"],
        MUTE_A
    );

    assert_eq!(
        core.execute(
            id,
            "set_channel_mute",
            params(json!({"channel": "A", "muted": true}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["A/Mute"]["muted"] == true).await;
    assert_eq!(
        core.execute(
            id,
            "set_channel_level",
            params(json!({"channel": "A", "gain_db": -3.0}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["A/Level"]["gain_db"] == -3.0).await;
    assert_eq!(
        core.execute(id, "set_power", params(json!({"powered": false})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["objects"]["Power"]["position"] == 0).await;
    let loaded = core
        .execute(id, "load_amp_preset", params(json!({"preset": 3})))
        .await;
    assert!(matches!(loaded, Ok(Outcome::Value { ref value }) if value["status"] == 0));

    let asked = log.lock().unwrap().clone();
    // The documented mute: SetState (4.2) on 0x10008205 with 1.
    assert!(asked.contains(&(MUTE_A, 4, 2, vec![1])));
    assert!(asked.contains(&(PRESETS, 3, 3, vec![3])));
    assert!(asked.contains(&(POWER, 4, 2, vec![0, 0])));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_port_defaults_by_model_and_is_required_for_a_ds100() {
    let core = common::core().unwrap();
    let opened = core.open(OpenRequest {
        device: "dbaudio".into(),
        model: "ds100".into(),
        host: "127.0.0.1".into(),
        port: None,
        settings: params(json!({})),
        monitor: false,
    });
    assert!(opened.is_err());
}
