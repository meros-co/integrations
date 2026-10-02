//! An ATEM switcher simulated on real UDP, driven through the public API.
//!
//! The simulator answers the hello, sends a small initial state from `_ver` to
//! `InCm` as reliable packets, acknowledges the client's packets, and applies
//! program changes by sending the resulting `PrgI` and `TlSr`. It deliberately
//! loses its first copy of one state packet, so the client has to leave the gap
//! and accept the resend.

#![cfg(feature = "blackmagic-atem")]

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const SESSION: u16 = 0x8123;

fn packet(flags: u8, session: u16, ack: u16, id: u16, payload: &[u8]) -> Vec<u8> {
    let length = 12 + payload.len() as u16;
    let mut p = Vec::new();
    p.extend_from_slice(&(((flags as u16) << 11) | length).to_be_bytes());
    p.extend_from_slice(&session.to_be_bytes());
    p.extend_from_slice(&ack.to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0]);
    p.extend_from_slice(&id.to_be_bytes());
    p.extend_from_slice(payload);
    p
}

fn cmd(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut c = ((8 + body.len()) as u16).to_be_bytes().to_vec();
    c.extend_from_slice(&[0xff, 0xff]);
    c.extend_from_slice(name);
    c.extend_from_slice(body);
    c
}

fn input(id: u16, long: &str, short: &str) -> Vec<u8> {
    let mut b = vec![0u8; 36];
    b[0..2].copy_from_slice(&id.to_be_bytes());
    b[2..2 + long.len()].copy_from_slice(long.as_bytes());
    b[22..22 + short.len()].copy_from_slice(short.as_bytes());
    cmd(b"InPr", &b)
}

fn tally(program: u16, preview: u16) -> Vec<u8> {
    let mut b = vec![0, 2];
    for source in [1u16, 2] {
        b.extend_from_slice(&source.to_be_bytes());
        b.push((source == program) as u8 | (((source == preview) as u8) << 1));
    }
    b.extend_from_slice(&[0, 0]);
    cmd(b"TlSr", &b)
}

async fn simulated_atem() -> u16 {
    let socket = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        let mut next_id: u16 = 1;
        let mut client = None;
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let data = buf[..n].to_vec();
            let flags = data[0] >> 3;
            if flags & 0x02 != 0 {
                // Hello: answer with the client's session id and status 2.
                client = Some(from);
                let reply = packet(0x02, 0x53ab, 0, 0, &[2, 0, 0, 0, 0, 0, 0, 0]);
                socket.send_to(&reply, from).await.unwrap();
                continue;
            }
            if flags == 0x10 && next_id == 1 && client == Some(from) {
                // The handshake's acknowledgement: send the initial state.
                let mut pin = vec![0u8; 44];
                pin[..9].copy_from_slice(b"ATEM Mini");
                pin[40] = 13;
                let mut top = vec![0u8; 24];
                top[0] = 1;
                top[1] = 2;
                top[2] = 1;
                top[3] = 1;
                let packets = [
                    [
                        cmd(b"_ver", &[0, 2, 0, 30]),
                        cmd(b"_pin", &pin),
                        cmd(b"_top", &top),
                    ]
                    .concat(),
                    [
                        cmd(b"_MeC", &[0, 1, 0, 0]),
                        input(1, "Camera 1", "CAM1"),
                        input(2, "Camera 2", "CAM2"),
                    ]
                    .concat(),
                    [
                        cmd(b"PrgI", &[0, 0, 0, 1]),
                        cmd(b"PrvI", &[0, 0, 0, 2, 0, 0, 0, 0]),
                        tally(1, 2),
                    ]
                    .concat(),
                    cmd(b"InCm", &[1, 0, 0, 0]),
                ];
                for (i, payload) in packets.iter().enumerate() {
                    let p = packet(0x01, SESSION, 0, next_id, payload);
                    next_id += 1;
                    // Lose the second packet's first copy: the client must
                    // drop what follows the gap until the resend arrives.
                    if i == 1 {
                        continue;
                    }
                    socket.send_to(&p, from).await.unwrap();
                }
                let resend = packet(0x01 | 0x04, SESSION, 0, 2, &packets[1]);
                socket.send_to(&resend, from).await.unwrap();
                for (i, payload) in packets.iter().enumerate().skip(2) {
                    let p = packet(0x01 | 0x04, SESSION, 0, i as u16 + 1, payload);
                    socket.send_to(&p, from).await.unwrap();
                }
                continue;
            }
            if flags & 0x01 != 0 {
                // A command: acknowledge it, then apply it.
                let id = u16::from_be_bytes([data[10], data[11]]);
                socket
                    .send_to(&packet(0x10, SESSION, id, 0, &[]), from)
                    .await
                    .unwrap();
                if &data[16..20] == b"CPgI" {
                    let source = u16::from_be_bytes([data[22], data[23]]);
                    let payload =
                        [cmd(b"PrgI", &[0, 0, 0, source as u8]), tally(source, 2)].concat();
                    let p = packet(0x01, SESSION, 0, next_id, &payload);
                    next_id += 1;
                    socket.send_to(&p, from).await.unwrap();
                }
            }
        }
    });
    port
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
async fn atem_end_to_end() {
    let port = simulated_atem().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "blackmagic-atem".into(),
            model: "atem-mini".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["product"], "ATEM Mini");
    assert_eq!(state["sources"]["2"]["long_name"], "Camera 2");
    assert_eq!(state["mes"]["1"]["program"], 1);
    assert_eq!(state["tally"]["1"]["program"], true);

    let outcome = core
        .execute(id, "set_program", params(json!({"me": 1, "source": 2})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(
        &core,
        |e| matches!(e, Event::State { patch, .. } if patch["mes"]["1"]["program"] == 2),
    )
    .await;
    assert_eq!(
        core.snapshot(id).unwrap().state["tally"]["2"]["program"],
        true
    );

    // A setting command passes the spec's checks and is acknowledged.
    let outcome = core
        .execute(id, "set_mix_rate", params(json!({"me": 1, "rate": 30})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    // The spec's range is checked before anything is sent.
    let outcome = core
        .execute(id, "set_mix_rate", params(json!({"me": 1, "rate": 0})))
        .await;
    assert!(matches!(outcome, Err(CommandError::InvalidParams { .. })));
    // The ATEM Mini model lists Fairlight commands, but this switcher reported
    // no Fairlight mixer, so the module refuses it.
    let outcome = core
        .execute(
            id,
            "set_fairlight_master",
            params(json!({"fader_gain": 0.0})),
        )
        .await;
    assert!(matches!(
        outcome,
        Err(CommandError::UnsupportedForModel { .. })
    ));
    // Classic audio is not listed for the ATEM Mini at all.
    let outcome = core
        .execute(id, "set_audio_master", params(json!({"gain": 0.0})))
        .await;
    assert!(matches!(
        outcome,
        Err(CommandError::UnsupportedForModel { .. })
    ));

    core.close(id).await;
}
