//! Art-Net against real loopback sockets, through the public API: a node
//! simulated as a UDP socket that answers ArtPoll and ArtAddress with an
//! ArtPollReply (Art-Net 4, revision 1.4dp) and receives the ArtDmx unicast
//! to it as a subscriber.

#![cfg(feature = "artnet")]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{Connection, Core, DeviceId, OpenRequest, Outcome};
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

/// An ArtPollReply for a node on 127.0.0.1 reached on `port`, with one
/// output port subscribed to universe 1.
fn poll_reply(port: u16, short_name: &str) -> Vec<u8> {
    let mut p = vec![0u8; 239];
    p[..8].copy_from_slice(b"Art-Net\0");
    p[8..10].copy_from_slice(&0x2100u16.to_le_bytes());
    p[10..14].copy_from_slice(&[127, 0, 0, 1]);
    p[14..16].copy_from_slice(&port.to_le_bytes());
    p[26..26 + short_name.len()].copy_from_slice(short_name.as_bytes());
    p[173] = 1; // one port
    p[174] = 0x80; // it outputs DMX512
    p[190] = 1; // SwOut: universe 1
    p[211] = 1;
    p
}

fn opcode(p: &[u8]) -> u16 {
    u16::from_le_bytes([p[8], p[9]])
}

/// The next packet with this OpCode the node receives.
async fn receive(node: &UdpSocket, wanted: u16) -> (Vec<u8>, SocketAddr) {
    let mut buf = vec![0u8; 2048];
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (n, from) = node.recv_from(&mut buf).await.unwrap();
            if n >= 10 && &buf[..8] == b"Art-Net\0" && opcode(&buf[..n]) == wanted {
                return (buf[..n].to_vec(), from);
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no packet with OpCode {wanted:#06x}"))
}

async fn state_until(core: &Core, device: DeviceId, done: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(device).unwrap().state;
            if done(&state) {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("state")
}

#[tokio::test(flavor = "multi_thread")]
async fn finds_a_node_sends_it_dmx_and_reprograms_it() {
    let core = common::core().unwrap();
    let node = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let node_port = node.local_addr().unwrap().port();
    let port = free_port();
    let device = core
        .open(OpenRequest {
            device: "artnet".into(),
            model: "artnet".into(),
            host: String::new(),
            port: Some(port),
            settings: params(json!({
                "bind_address": "127.0.0.1",
                "poll_addresses": format!("127.0.0.1:{node_port}"),
                "refresh_ms": 0,
            })),
            monitor: true,
        })
        .unwrap();

    // The node is polled and answers.
    let (_, controller) = receive(&node, 0x2000).await;
    assert_eq!(controller.port(), port);
    node.send_to(&poll_reply(node_port, "Gateway"), controller)
        .await
        .unwrap();
    let state = state_until(&core, device, |s| {
        s["nodes"]["127.0.0.1/1"]["online"] == true
    })
    .await;
    assert_eq!(state["nodes"]["127.0.0.1/1"]["short_name"], "Gateway");
    assert_eq!(
        state["nodes"]["127.0.0.1/1"]["ports"]["1"]["output_universe"],
        1
    );
    assert_eq!(
        core.snapshot(device).unwrap().connection,
        Connection::Connected
    );

    // DMX for universe 1 goes to the subscriber.
    let outcome = core
        .execute(
            device,
            "set_channel",
            params(json!({"universe": 1, "channel": 1, "value": 200})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    let (dmx, _) = receive(&node, 0x5000).await;
    assert_eq!(&dmx[14..18], &[1, 0, 2, 0], "universe 1, 512 channels");
    assert_eq!(dmx[18], 200);

    // Renaming completes on the node's ArtPollReply, which reads it back.
    let (rename, ()) = tokio::join!(
        core.execute(
            device,
            "set_node_names",
            params(json!({"node": "127.0.0.1", "short_name": "Stage left"})),
        ),
        async {
            let (address, from) = receive(&node, 0x6000).await;
            assert_eq!(&address[14..24], b"Stage left");
            node.send_to(&poll_reply(node_port, "Stage left"), from)
                .await
                .unwrap();
        }
    );
    assert_eq!(rename, Ok(Outcome::Ack));
    let state = state_until(&core, device, |s| {
        s["nodes"]["127.0.0.1/1"]["short_name"] == "Stage left"
    })
    .await;
    assert!(core.snapshot(device).unwrap().latency_ms.is_some());
    assert_eq!(state["outputs"]["1"]["channels"][0], 200);
}
