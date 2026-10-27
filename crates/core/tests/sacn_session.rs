//! sACN against real loopback sockets, through the public API, by unicast
//! (ANSI E1.31-2016 §9.1: receivers also take data on their unicast
//! address), so nothing leaves the host: a receiver simulated as a UDP
//! socket gets the core's output, and a source simulated the same way sends
//! a universe the core receives.

#![cfg(feature = "sacn")]

mod common;

use std::time::Duration;

use meros_integrations::{OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// An E1.31 Data Packet (Table 1) for universe 1 with two slots.
fn data_packet(sequence: u8, slots: [u8; 2]) -> Vec<u8> {
    let mut p = vec![0x00, 0x10, 0x00, 0x00];
    p.extend_from_slice(b"ASC-E1.17\0\0\0");
    p.extend_from_slice(&[0x70, 0x6f]); // root length 127 - 16 + 16
    p.extend_from_slice(&4u32.to_be_bytes());
    p.extend_from_slice(&[0x11; 16]); // CID
    p.extend_from_slice(&[0x70, 0x59]);
    p.extend_from_slice(&2u32.to_be_bytes());
    let mut name = b"Desk".to_vec();
    name.resize(64, 0);
    p.extend_from_slice(&name);
    p.extend_from_slice(&[100, 0, 0, sequence, 0, 0, 1]);
    p.extend_from_slice(&[0x70, 0x0d, 0x02, 0xa1, 0, 0, 0, 1, 0, 3, 0]);
    p.extend_from_slice(&slots);
    let total = p.len();
    p[16..18].copy_from_slice(&(0x7000u16 | (total - 16) as u16).to_be_bytes());
    p[38..40].copy_from_slice(&(0x7000u16 | (total - 38) as u16).to_be_bytes());
    p
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_and_receives_universes_by_unicast() {
    let core = common::core().unwrap();
    let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let peer_port = peer.local_addr().unwrap().port();
    let port = free_port();
    let device = core
        .open(OpenRequest {
            device: "sacn".into(),
            model: "sacn".into(),
            host: String::new(),
            port: Some(port),
            settings: params(json!({
                "bind_address": "127.0.0.1",
                "multicast": false,
                "unicast_addresses": format!("127.0.0.1:{peer_port}"),
                "receive": "1",
                "receive_interval_ms": 0,
                "cid": "ef07c8dd-0064-4401-a3a2-459ef8e6143e",
            })),
            monitor: true,
        })
        .unwrap();

    // Output reaches the receiver.
    let outcome = core
        .execute(
            device,
            "set_channel",
            params(json!({"universe": 7, "channel": 2, "value": 77})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    let mut buf = vec![0u8; 1024];
    let (n, from) = tokio::time::timeout(Duration::from_secs(10), peer.recv_from(&mut buf))
        .await
        .expect("a data packet")
        .unwrap();
    assert_eq!(from.port(), port);
    assert_eq!(n, 638);
    assert_eq!(&buf[113..115], &[0, 7], "universe 7");
    assert_eq!(buf[127], 77, "channel 2");

    // A source's universe is received.
    peer.send_to(&data_packet(1, [12, 34]), ("127.0.0.1", port))
        .await
        .unwrap();
    let state = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(device).unwrap().state;
            if state["inputs"]["1"]["channels"] == json!([12, 34]) {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the universe is received");
    let source = &state["inputs"]["1"]["sources"]["11111111-1111-1111-1111-111111111111"];
    assert_eq!(source["name"], "Desk");
    assert_eq!(source["priority"], 100);
    assert_eq!(state["outputs"]["7"]["channels"][1], 77);
}
