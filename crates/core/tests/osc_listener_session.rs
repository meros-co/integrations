//! The OSC listener against real loopback sockets, through the public API:
//! control surfaces simulated as UDP senders and TCP clients, OSC 1.0
//! messages and bundles (Wright 2002), OSC 1.1 SLIP framing over TCP.

#![cfg(feature = "osc-listener")]

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{Connection, Core, DeviceId, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn osc_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// An OSC message with int32 arguments.
fn message(address: &str, ints: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, address);
    osc_string(&mut out, &format!(",{}", "i".repeat(ints.len())));
    for n in ints {
        out.extend_from_slice(&n.to_be_bytes());
    }
    out
}

fn bundle(elements: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"#bundle\0".to_vec();
    out.extend_from_slice(&1u64.to_be_bytes()); // "immediately"
    for e in elements {
        out.extend_from_slice(&(e.len() as i32).to_be_bytes());
        out.extend_from_slice(e);
    }
    out
}

fn slip(packet: &[u8]) -> Vec<u8> {
    let mut out = vec![0xC0];
    for &b in packet {
        match b {
            0xC0 => out.extend_from_slice(&[0xDB, 0xDC]),
            0xDB => out.extend_from_slice(&[0xDB, 0xDD]),
            b => out.push(b),
        }
    }
    out.push(0xC0);
    out
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn open(core: &Core, port: u16, settings: Value) -> DeviceId {
    core.open(OpenRequest {
        device: "osc-listener".into(),
        model: "generic".into(),
        // No host: it hears any sender.
        host: String::new(),
        port: Some(port),
        settings: params(settings),
        monitor: false,
    })
    .unwrap()
}

async fn wait_connected(core: &Core, device: DeviceId) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while core.snapshot(device).unwrap().connection != Connection::Connected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("listening");
}

/// Message events as (address, args, source), until `n` have arrived.
async fn messages(core: &Core, n: usize) -> Vec<(String, Value, String)> {
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while out.len() < n {
            for e in core.next_events(1024).await {
                if let Event::Message {
                    address,
                    args,
                    source,
                    ..
                } = e
                {
                    out.push((address, args, source));
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{n} messages, got {out:?}"));
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn udp_from_any_sender_every_message_and_bundles_in_order() {
    let core = Core::new().unwrap();
    let port = free_port();
    let device = open(&core, port, json!({"bind_address": "127.0.0.1"}));
    wait_connected(&core, device).await;
    let to: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let press = message("/1/push1", &[1]);
    // The same message twice: a button pressed twice.
    a.send_to(&press, to).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    a.send_to(&press, to).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    b.send_to(&message("/2/fader1", &[7]), to).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let nested = bundle(&[message("/b/2", &[2]), message("/b/3", &[3])]);
    b.send_to(&bundle(&[message("/b/1", &[1]), nested]), to)
        .await
        .unwrap();

    let got = messages(&core, 6).await;
    let a_addr = a.local_addr().unwrap().to_string();
    let b_addr = b.local_addr().unwrap().to_string();
    assert_eq!(
        got,
        [
            ("/1/push1".to_string(), json!([1]), a_addr.clone()),
            ("/1/push1".to_string(), json!([1]), a_addr.clone()),
            ("/2/fader1".to_string(), json!([7]), b_addr.clone()),
            ("/b/1".to_string(), json!([1]), b_addr.clone()),
            ("/b/2".to_string(), json!([2]), b_addr.clone()),
            ("/b/3".to_string(), json!([3]), b_addr.clone()),
        ]
    );
    let state = core.snapshot(device).unwrap().state;
    assert_eq!(state["message_count"], 6);
    assert_eq!(state["senders"][&a_addr]["messages"], 2);
    assert_eq!(state["senders"][&b_addr]["last_address"], "/b/3");

    // Feedback to a sender, from the listening port.
    let outcome = core
        .execute(
            device,
            "send",
            params(json!({"to": a_addr, "address": "/1/push1/color", "args": ["red"]})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    let mut buf = [0u8; 256];
    let (n, from) = tokio::time::timeout(Duration::from_secs(5), a.recv_from(&mut buf))
        .await
        .expect("feedback arrives")
        .unwrap();
    assert_eq!(from.port(), port);
    assert!(buf[..n].starts_with(b"/1/push1/color\0\0,s\0\0red\0"));

    // A second listener cannot take the same port: disconnected, not a panic.
    let clash = open(&core, port, json!({"bind_address": "127.0.0.1"}));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            core.snapshot(clash).unwrap().connection,
            Connection::Disconnected { .. }
        ) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the clash is reported");
}

#[tokio::test(flavor = "multi_thread")]
async fn tcp_slip_two_clients_at_once_and_garbage_closes_only_its_sender() {
    let core = Core::new().unwrap();
    let port = free_port();
    let device = open(
        &core,
        port,
        json!({"transport": "tcp", "bind_address": "127.0.0.1", "address_prefix": "/x"}),
    );
    wait_connected(&core, device).await;
    let to = format!("127.0.0.1:{port}");

    let mut one = TcpStream::connect(&to).await.unwrap();
    let mut two = TcpStream::connect(&to).await.unwrap();
    // A message split across writes, and one the prefix filters out.
    let first = slip(&message("/x/one", &[1]));
    one.write_all(&first[..5]).await.unwrap();
    two.write_all(&slip(&message("/x/two", &[2])))
        .await
        .unwrap();
    two.write_all(&slip(&message("/y/skip", &[0])))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    one.write_all(&first[5..]).await.unwrap();
    let got = messages(&core, 2).await;
    let one_addr = one.local_addr().unwrap().to_string();
    let two_addr = two.local_addr().unwrap().to_string();
    assert!(got.contains(&("/x/one".into(), json!([1]), one_addr.clone())));
    assert!(got.contains(&("/x/two".into(), json!([2]), two_addr.clone())));

    // Bytes that are not OSC are skipped; the connection stays.
    two.write_all(&slip(b"not osc at all")).await.unwrap();
    two.write_all(&slip(&message("/x/two", &[3])))
        .await
        .unwrap();
    assert_eq!(
        messages(&core, 1).await,
        [("/x/two".into(), json!([3]), two_addr.clone())]
    );

    // Over 1 MiB with no END byte: that sender is closed, nobody else.
    let junk = vec![0x41u8; (1 << 20) + 16];
    let _ = one.write_all(&junk).await;
    let mut buf = [0u8; 16];
    let closed = tokio::time::timeout(Duration::from_secs(5), one.read(&mut buf)).await;
    assert!(
        matches!(closed, Ok(Ok(0)) | Ok(Err(_))),
        "the garbled sender is closed"
    );
    two.write_all(&slip(&message("/x/two", &[4])))
        .await
        .unwrap();
    assert_eq!(
        messages(&core, 1).await,
        [("/x/two".into(), json!([4]), two_addr.clone())]
    );
    // A new sender is still accepted.
    let mut three = TcpStream::connect(&to).await.unwrap();
    three
        .write_all(&slip(&message("/x/three", &[5])))
        .await
        .unwrap();
    assert_eq!(messages(&core, 1).await[0].0, "/x/three");

    // Feedback to a connected sender, framed as it sends.
    let outcome = core
        .execute(
            device,
            "send",
            params(json!({"to": two_addr, "address": "/x/led", "args": [1]})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    let mut buf = vec![0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(5), two.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&buf[..n], slip(&message("/x/led", &[1])).as_slice());
    let state = core.snapshot(device).unwrap().state;
    assert_eq!(state["senders"][&one_addr]["connected"], false);
    assert_eq!(state["senders"][&two_addr]["connected"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn tcp_length_prefixed_framing() {
    let core = Core::new().unwrap();
    let port = free_port();
    let device = open(
        &core,
        port,
        json!({"transport": "tcp", "tcp_framing": "length-prefixed", "bind_address": "127.0.0.1"}),
    );
    wait_connected(&core, device).await;
    let mut s = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let packet = message("/len", &[9]);
    let mut framed = (packet.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(&packet);
    s.write_all(&framed).await.unwrap();
    assert_eq!(messages(&core, 1).await[0].1, json!([9]));
}

/// A consumer that drains nothing while a surface sends more than the
/// queue holds: state patches are dropped, every message event is kept.
#[tokio::test(flavor = "multi_thread")]
async fn overflow_keeps_every_message() {
    const N: i32 = 10_500;
    let core = Core::new().unwrap();
    let port = free_port();
    let device = open(
        &core,
        port,
        json!({"transport": "tcp", "bind_address": "127.0.0.1"}),
    );
    wait_connected(&core, device).await;
    let mut s = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let mut stream = Vec::new();
    for i in 0..N {
        stream.extend(slip(&message("/1/push1", &[i % 2])));
    }
    s.write_all(&stream).await.unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        while core.snapshot(device).unwrap().state["message_count"] != N {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("every message is received");

    let mut dropped = 0;
    let mut args = Vec::new();
    loop {
        let events = core.poll_events(4096);
        if events.is_empty() {
            break;
        }
        for e in events {
            match e {
                Event::Dropped { count, messages } => {
                    assert_eq!(messages, 0);
                    dropped += count;
                }
                Event::Message { args: a, .. } => args.push(a),
                _ => {}
            }
        }
    }
    assert!(dropped > 0, "state patches were dropped");
    assert_eq!(args.len(), N as usize);
    assert!(args.iter().enumerate().all(|(i, a)| *a == json!([i % 2])));
}

#[test]
fn opened_without_a_host_through_the_json_surface() {
    let core = Core::new().unwrap();
    let port = free_port();
    let opened = meros_integrations::json::open(
        &core,
        &json!({"device": "osc-listener", "model": "generic", "port": port,
                "settings": {"bind_address": "127.0.0.1"}}),
    );
    assert!(opened["device"].is_u64(), "{opened}");
    // Without a port there is nothing to listen on.
    let refused = meros_integrations::json::open(
        &core,
        &json!({"device": "osc-listener", "model": "generic"}),
    );
    assert_eq!(refused["error"]["error"], "not_implemented");
}
