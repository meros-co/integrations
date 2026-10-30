//! A KissBox IO8CC simulated as a loopback UDP socket, driven through the
//! public API. The simulator holds a digital output card in slot 1 and
//! answers read all channels (A0h) with all channel status (A1h) and read one
//! channel (A2h) with single channel status (A3h), and applies write one
//! channel (A5h), as KissBox's TCP/UDP control protocol V2 lays them out. It
//! sends its answers to the notification port, as the frame does.

#![cfg(feature = "kissbox-io")]

mod common;

use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{OpenRequest, Outcome};
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

#[tokio::test(flavor = "multi_thread")]
async fn reads_and_writes_a_card_through_the_public_api() {
    let frame = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let frame_port = frame.local_addr().unwrap().port();
    let feedback = free_port();
    tokio::spawn(async move {
        let mut contacts = [0u8; 8];
        let mut buf = [0u8; 64];
        loop {
            let Ok((n, _)) = frame.recv_from(&mut buf).await else {
                continue;
            };
            let reply = match &buf[..n] {
                [0xA0, 0] => {
                    let mut r = vec![0xA1, 0];
                    r.extend(contacts);
                    r
                }
                [0xA2, 0, c] => vec![0xA3, 0, *c, contacts[usize::from(*c)]],
                [0xA5, 0, c, v] => {
                    contacts[usize::from(*c)] = *v;
                    continue;
                }
                _ => continue,
            };
            let _ = frame.send_to(&reply, ("127.0.0.1", feedback)).await;
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "kissbox-io".into(),
            model: "io8cc".into(),
            host: "127.0.0.1".into(),
            port: Some(frame_port),
            settings: params(json!({
                "slots": "do",
                "feedback_port": feedback,
                "bind_address": "127.0.0.1",
                "poll_interval_ms": 200,
            })),
            monitor: true,
        })
        .unwrap();

    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let outcome = core
                .execute(
                    device,
                    "set_output",
                    params(json!({"slot": 1, "channel": 5, "active": true})),
                )
                .await;
            if outcome.is_ok() {
                return outcome;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("connected");
    assert_eq!(outcome, Ok(Outcome::Unverified));

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(device).unwrap().state;
            if state["slots"]["1"]["channels"]["5"]["on"] == json!(true) {
                assert_eq!(state["slots"]["1"]["card"], "digital_output");
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the write is read back");

    let outcome = core
        .execute(
            device,
            "read_channel",
            params(json!({"slot": 1, "channel": 5})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Value { value: json!(1) }));
    core.close(device).await;
}
