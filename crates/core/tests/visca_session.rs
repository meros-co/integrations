//! A Sony VISCA over IP camera simulated on real UDP, driven through the
//! public API.
//!
//! The simulator answers RESET, answers inquiries from a small table (syntax
//! error for the rest), acknowledges commands and completes them a little
//! later in socket 1, refuses manual focus as "not executable", and drops the
//! first copy of a preset recall so the client has to send it again with the
//! same sequence number. It records every sequence number it receives.

#![cfg(feature = "visca")]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

fn frame(kind: u16, seq: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = kind.to_be_bytes().to_vec();
    v.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    v.extend_from_slice(&seq.to_be_bytes());
    v.extend_from_slice(payload);
    v
}

/// (payload type, sequence number, VISCA payload) of every message received.
type Log = Arc<Mutex<Vec<(u16, u32, Vec<u8>)>>>;

async fn simulated_camera() -> (u16, Log) {
    let socket = Arc::new(
        UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap(),
    );
    let port = socket.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 256];
        let mut dropped_recall = false;
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let data = &buf[..n];
            let kind = u16::from_be_bytes([data[0], data[1]]);
            let len = u16::from_be_bytes([data[2], data[3]]) as usize;
            let seq = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            let payload = data[8..8 + len].to_vec();
            record.lock().unwrap().push((kind, seq, payload.clone()));
            let reply = |p: &[u8]| frame(0x0111, seq, p);
            match kind {
                // RESET: acknowledged with 01.
                0x0200 if payload == [0x01] => {
                    let _ = socket.send_to(&frame(0x0201, seq, &[0x01]), from).await;
                }
                0x0110 => {
                    let answer: &[u8] = match &payload[1..payload.len() - 1] {
                        [0x09, 0x00, 0x02] => {
                            &[0x90, 0x50, 0x00, 0x01, 0x06, 0x17, 0x01, 0x00, 0x02, 0xFF]
                        }
                        [0x09, 0x04, 0x00] => &[0x90, 0x50, 0x02, 0xFF],
                        [0x09, 0x04, 0x47] => &[0x90, 0x50, 0x04, 0x00, 0x00, 0x00, 0xFF],
                        [0x09, 0x04, 0x38] => &[0x90, 0x50, 0x02, 0xFF],
                        [0x09, 0x06, 0x12] => &[
                            0x90, 0x50, 0x0D, 0x0E, 0x00, 0x00, 0x01, 0x02, 0x00, 0x00, 0xFF,
                        ],
                        _ => &[0x90, 0x60, 0x02, 0xFF],
                    };
                    let _ = socket.send_to(&reply(answer), from).await;
                }
                0x0100 => {
                    let body = &payload[1..payload.len() - 1];
                    if body.starts_with(&[0x01, 0x04, 0x48]) {
                        // Manual focus position while in auto focus.
                        let _ = socket
                            .send_to(&reply(&[0x90, 0x60, 0x41, 0xFF]), from)
                            .await;
                        continue;
                    }
                    if body.starts_with(&[0x01, 0x04, 0x3F, 0x02]) && !dropped_recall {
                        dropped_recall = true;
                        continue;
                    }
                    let _ = socket.send_to(&reply(&[0x90, 0x41, 0xFF]), from).await;
                    let socket = socket.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        let _ = socket
                            .send_to(&frame(0x0111, seq, &[0x90, 0x51, 0xFF]), from)
                            .await;
                    });
                }
                _ => {}
            }
        }
    });
    (port, log)
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
async fn visca_over_ip_end_to_end() {
    let (port, log) = simulated_camera().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "visca".into(),
            model: "sony-visca-ip".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();

    // Polled positions arrive as state.
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["pan_tilt"]["pan"] == -0x2200)
    })
    .await;
    wait_for(&core, |_| {
        let s = core.snapshot(id).unwrap().state;
        s["zoom"]["position"] == 0x4000 && s["device"]["model_id"] == 0x0617
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["pan_tilt"]["tilt"], 0x1200);

    // An inquiry as a command.
    assert_eq!(
        core.execute(id, "get_power", params(json!({}))).await,
        Ok(Outcome::Value { value: json!("on") })
    );

    // ACK then Completion.
    assert_eq!(
        core.execute(id, "zoom_to", params(json!({"position": 0x2000})))
            .await,
        Ok(Outcome::Ack)
    );

    // The first copy is lost: sent again with the same sequence number.
    assert_eq!(
        core.execute(id, "preset_recall", params(json!({"preset": 3})))
            .await,
        Ok(Outcome::Ack)
    );

    // Error 41: not executable in auto focus.
    match core
        .execute(id, "focus_to", params(json!({"position": 0x1000})))
        .await
    {
        Err(CommandError::DeviceError { code, .. }) => assert_eq!(code.as_deref(), Some("41")),
        other => panic!("expected a device error, got {other:?}"),
    }

    // Out-of-range for the model is refused before sending.
    assert!(matches!(
        core.execute(id, "preset_recall", params(json!({"preset": 200})))
            .await,
        Err(CommandError::InvalidParams { .. })
    ));

    let log = log.lock().unwrap().clone();
    // RESET first, then numbers from 1, rising by one except for the resend.
    assert_eq!(log[0].0, 0x0200);
    assert_eq!(log[1].1, 1);
    let recalls: Vec<_> = log
        .iter()
        .filter(|(k, _, p)| *k == 0x0100 && p.starts_with(&[0x81, 0x01, 0x04, 0x3F, 0x02]))
        .collect();
    assert_eq!(recalls.len(), 2, "sent twice");
    assert_eq!(recalls[0].1, recalls[1].1, "with the same sequence number");
    assert_eq!(recalls[0].2, [0x81, 0x01, 0x04, 0x3F, 0x02, 0x02, 0xFF]);
    let mut previous = 0;
    for (kind, seq, payload) in &log[1..] {
        let resend = *kind == 0x0100 && payload.starts_with(&[0x81, 0x01, 0x04, 0x3F, 0x02]);
        if resend && *seq == previous {
            continue;
        }
        assert_eq!(*seq, previous + 1, "sequence numbers rise by one");
        previous = *seq;
    }

    core.close(id).await;
}
