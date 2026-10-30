//! generic-ping through the public API against loopback: ICMP echo where
//! the system allows it without privileges (skipped, saying so, where it
//! does not), a TCP check against a local listener and against a port
//! nobody listens on, and Wake-on-LAN received on a local UDP socket.

#![cfg(feature = "generic-ping")]

mod common;

use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Connection, Core, DeviceId, OpenRequest, Outcome};

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: Option<u16>, settings: Value) -> DeviceId {
    core.open(OpenRequest {
        device: "generic-ping".into(),
        model: "generic".into(),
        host: "127.0.0.1".into(),
        port,
        settings: params(settings),
        monitor: true,
    })
    .unwrap()
}

async fn wait_for_state(core: &Core, id: DeviceId, pred: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if pred(&state) {
                return state;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn icmp_echo_to_loopback_is_answered_where_the_system_allows_it() {
    let core = common::core().unwrap();
    let id = open(&core, None, json!({ "method": "icmp", "interval_ms": 200 }));
    let state = wait_for_state(&core, id, |s| {
        s["reachable"] == true
            || s["last_error"]
                .as_str()
                .is_some_and(|e| e.contains("not allowed"))
    })
    .await;
    if state["reachable"] != true {
        eprintln!("skipped: {}", state["last_error"]);
        return;
    }
    assert_eq!(state["method"], "icmp");
    assert!(state["rtt_ms"].as_f64().unwrap() < 1000.0);
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);
    // A ping now answers with the probe's result.
    let out = core.execute(id, "ping", params(json!({}))).await.unwrap();
    match out {
        Outcome::Value { value } => {
            assert_eq!(value["reachable"], true);
            assert_eq!(value["method"], "icmp");
        }
        other => panic!("{other:?}"),
    }
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn tcp_checks_a_listener_and_counts_a_refusal_as_an_answer() {
    let core = common::core().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
    let id = open(
        &core,
        Some(port),
        json!({ "method": "tcp", "interval_ms": 200 }),
    );
    let state = wait_for_state(&core, id, |s| s["received"].as_u64() >= Some(2)).await;
    assert_eq!(state["reachable"], true);
    assert_eq!(state["method"], "tcp");
    assert_eq!(state["loss_percent"], 0.0);
    core.close(id).await;

    // A port nobody listens on: the host's reset is an answer.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let id = open(
        &core,
        Some(port),
        json!({ "method": "tcp", "timeout_ms": 5000 }),
    );
    let state = wait_for_state(&core, id, |s| {
        s["reachable"] == true || s["consecutive_failures"].as_u64() > Some(0)
    })
    .await;
    assert_eq!(state["reachable"], true, "{state}");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn wake_on_lan_sends_the_magic_packet() {
    let core = common::core().unwrap();
    let receiver = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let wol_port = receiver.local_addr().unwrap().port();
    let id = open(
        &core,
        Some(1),
        json!({
            "method": "tcp",
            "interval_ms": 86400000,
            "mac_address": "00:11:22:33:44:55",
            "secureon_password": "0102030405ff",
            "wol_address": "127.0.0.1",
            "wol_port": wol_port,
        }),
    );
    let out = core
        .execute(id, "wake_on_lan", params(json!({})))
        .await
        .unwrap();
    assert_eq!(out, Outcome::Unverified);
    let mut buf = [0u8; 256];
    let (n, _) = tokio::time::timeout(Duration::from_secs(5), receiver.recv_from(&mut buf))
        .await
        .expect("a magic packet")
        .unwrap();
    assert_eq!(n, 108);
    assert_eq!(&buf[..6], &[0xFF; 6]);
    assert_eq!(&buf[6..12], &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    assert_eq!(&buf[96..102], &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    assert_eq!(&buf[102..108], &[1, 2, 3, 4, 5, 0xFF]);
    // Another MAC address given with the command.
    core.execute(
        id,
        "wake_on_lan",
        params(json!({ "mac_address": "aa-bb-cc-dd-ee-ff" })),
    )
    .await
    .unwrap();
    let (n, _) = tokio::time::timeout(Duration::from_secs(5), receiver.recv_from(&mut buf))
        .await
        .expect("a magic packet")
        .unwrap();
    assert_eq!(n, 108);
    assert_eq!(&buf[6..12], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    core.close(id).await;
}
