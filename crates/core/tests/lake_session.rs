//! A Lab.gruppen D Series frame simulated on real UDP in DLM's dynamic answer
//! port mode, driven through the public API.
//!
//! The simulator answers `Dev.Network.ID?` with its frame id and from that
//! id, answers gets from a small table, acknowledges sets with -2 (and an
//! unknown path with -6), keeps the output gain it is given, and answers the
//! version 3 meter request with a structure whose amp status says the frame
//! is on. It records every message id it receives, to check they rise.

#![cfg(feature = "labgruppen-lake")]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const FRAME: (u32, u32) = (0x3d00_0011, 0xd6ed_9201);

fn packet(kind: u16, msg_id: u32, dest: (u32, u32), payload: &[u8]) -> Vec<u8> {
    let length = 28 + payload.len() + 4;
    let mut p = Vec::new();
    for v in [FRAME.0, FRAME.1, dest.0, dest.1] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p.extend_from_slice(&5u16.to_le_bytes());
    p.extend_from_slice(&6u16.to_le_bytes());
    p.extend_from_slice(&(length as u16).to_le_bytes());
    p.extend_from_slice(&kind.to_le_bytes());
    p.extend_from_slice(&msg_id.to_le_bytes());
    p.extend_from_slice(payload);
    p.extend_from_slice(&[0; 4]);
    p
}

#[derive(Default)]
struct Frame {
    msg_ids: Vec<u32>,
    texts: Vec<String>,
    gain: String,
}

async fn simulated_frame() -> (u16, Arc<Mutex<Frame>>) {
    let socket = Arc::new(
        UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap(),
    );
    let port = socket.local_addr().unwrap().port();
    let state = Arc::new(Mutex::new(Frame {
        gain: "0.00".into(),
        ..Frame::default()
    }));
    let frame = state.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 600];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let d = &buf[..n];
            let src = (
                u32::from_le_bytes([d[0], d[1], d[2], d[3]]),
                u32::from_le_bytes([d[4], d[5], d[6], d[7]]),
            );
            let msg_id = u32::from_le_bytes([d[24], d[25], d[26], d[27]]);
            let payload = &d[28..n - 4];
            let end = payload
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(payload.len());
            let text = String::from_utf8_lossy(&payload[..end]).to_string();
            let reply = {
                let mut f = frame.lock().unwrap();
                f.msg_ids.push(msg_id);
                f.texts.push(text.clone());
                let get = |v: &str| {
                    let mut b = v.as_bytes().to_vec();
                    b.push(0);
                    packet(701, msg_id, src, &b)
                };
                let ack = |code: i32| packet(2, msg_id, src, &code.to_le_bytes());
                if let Some(value) = text.strip_prefix("Mod.Out.Gain=A 1 ") {
                    f.gain = value.to_string();
                    ack(-2)
                } else {
                    match text.as_str() {
                        "Dev.Network.ID?" => get("3d000011:d6ed9201"),
                        "Dev.ModelName?" => get("D80:4L"),
                        "Dev.Power?" => get("1"),
                        "Dev.MD.NoFaults?" => get("1"),
                        "Mod.Out.Chans?A" => get("1"),
                        "Mod.Out.Chans?B" | "Mod.Out.Chans?C" | "Mod.Out.Chans?D" => get("0"),
                        "Mod.Out.Gain?A 1" => get(&format!("{} {} -100.00 20.00", f.gain, f.gain)),
                        "Dev.MD.FullBin?3" => {
                            let mut m = vec![0u8; 108];
                            m[0] = 0x01; // power on
                            m[72] = 0xFE; // input 1 peak -0.5 dBFS
                            packet(701, msg_id, src, &m)
                        }
                        t if t.contains('?') => get("0"),
                        t if t.starts_with("Bogus") => ack(-6),
                        _ => ack(-2),
                    }
                }
            };
            let _ = socket.send_to(&reply, from).await;
        }
    });
    (port, state)
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
async fn lake_end_to_end() {
    let (port, frame) = simulated_frame().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "labgruppen-lake".into(),
            model: "d-80-4l".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"meter_poll_ms": 200, "parameter_poll_ms": 500})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;

    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["frame_id"], "3d000011:d6ed9201");
    assert_eq!(state["device"]["model_name"], "D80:4L");
    assert_eq!(state["device"]["power"], true);
    assert_eq!(state["modules"]["A"]["output_channels"], 1);
    assert_eq!(state["modules"]["A"]["outputs"]["1"]["gain_db"], 0.0);
    assert_eq!(state["meters"]["amp"]["power_on"], true);
    assert_eq!(state["meters"]["inputs"]["1"]["peak_dbfs"], -0.5);

    let outcome = core
        .execute(
            id,
            "set_output_gain",
            params(json!({"module": "A", "channel": 1, "gain_db": -6.5})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    assert_eq!(frame.lock().unwrap().gain, "-6.50");
    assert_eq!(
        core.snapshot(id).unwrap().state["modules"]["A"]["outputs"]["1"]["gain_db"],
        -6.5
    );

    let outcome = core
        .execute(
            id,
            "send_message",
            params(json!({"message": "Dev.Out.Route?PC 1 Analog"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Value { value: json!(0.0) }));

    let outcome = core
        .execute(
            id,
            "set_module_selected",
            params(json!({"module": "E", "selected": true})),
        )
        .await;
    assert!(matches!(outcome, Err(CommandError::InvalidParams { .. })));

    // Every packet carried a new, rising message id.
    let ids = frame.lock().unwrap().msg_ids.clone();
    assert!(ids.windows(2).all(|w| w[1] > w[0]), "{ids:?}");
    core.close(id).await;
}
