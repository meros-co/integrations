//! PosiStageNet against real loopback sockets, through the public API, by
//! unicast so nothing leaves the host: a tracking server simulated as a UDP
//! socket sends PSN_INFO and PSN_DATA (protocol v2.03) that the core keeps,
//! and receives the trackers the core sends as a server.

#![cfg(feature = "posistagenet")]

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

/// A chunk: id, data length and the subchunk bit in a little-endian word.
fn chunk(id: u16, data: &[u8], sub: bool) -> Vec<u8> {
    let header = id as u32 | ((data.len() as u32) << 16) | ((sub as u32) << 31);
    let mut out = header.to_le_bytes().to_vec();
    out.extend_from_slice(data);
    out
}

fn header(frame: u8) -> Vec<u8> {
    let mut d = 1000u64.to_le_bytes().to_vec();
    d.extend_from_slice(&[2, 3, frame, 1]);
    chunk(0, &d, false)
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_a_servers_trackers_and_sends_its_own() {
    let core = common::core().unwrap();
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_port = server.local_addr().unwrap().port();
    let port = free_port();
    let device = core
        .open(OpenRequest {
            device: "posistagenet".into(),
            model: "posistagenet".into(),
            host: String::new(),
            port: Some(port),
            settings: params(json!({
                "bind_address": "127.0.0.1",
                "multicast": false,
                "unicast_addresses": format!("127.0.0.1:{server_port}"),
                "receive_interval_ms": 0,
            })),
            monitor: true,
        })
        .unwrap();

    // PSN_INFO names tracker 4; PSN_DATA gives its position.
    let mut info = header(1);
    info.extend(chunk(1, b"Tracking", false));
    info.extend(chunk(2, &chunk(4, &chunk(0, b"Actor", false), true), true));
    let info = chunk(0x6756, &info, true);
    let mut pos = Vec::new();
    for f in [1.0f32, 2.5, -3.0] {
        pos.extend_from_slice(&f.to_le_bytes());
    }
    let mut data = header(1);
    data.extend(chunk(1, &chunk(4, &chunk(0, &pos, false), true), true));
    let data = chunk(0x6755, &data, true);
    let to = ("127.0.0.1", port);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            server.send_to(&info, to).await.unwrap();
            server.send_to(&data, to).await.unwrap();
            let state = core.snapshot(device).unwrap().state;
            let tracker = &state["servers"]["127.0.0.1"]["trackers"]["4"];
            if tracker["position"]["y"] == json!(2.5) && tracker["name"] == "Actor" {
                assert_eq!(state["servers"]["127.0.0.1"]["system_name"], "Tracking");
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the tracker is kept");

    // The core's own tracker reaches the server.
    let outcome = core
        .execute(
            device,
            "set_tracker",
            params(json!({"tracker": 9, "position_x": 4.0, "position_y": 0.0, "position_z": 1.0})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    let mut buf = vec![0u8; 1500];
    // Windows reports a port-unreachable answer to an earlier send (one
    // before the core was listening) as a receive error: not this packet.
    let (n, from) = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match server.recv_from(&mut buf).await {
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                other => return other.unwrap(),
            }
        }
    })
    .await
    .expect("a PSN packet");
    assert_eq!(from.port(), port);
    assert!(n > 4);
    let root = u16::from_le_bytes([buf[0], buf[1]]);
    assert!(root == 0x6755 || root == 0x6756);
}
