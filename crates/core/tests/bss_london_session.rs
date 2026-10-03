//! A BSS Soundweb London simulated on real TCP, driven through the public API.
//!
//! The simulator answers every SUBSCRIBE with a SET carrying a fixed value,
//! as the Interface Kit describes, and records what it received.

#![cfg(feature = "bss-london")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = vec![0x02];
    let mut sum = 0u8;
    let mut bytes = body.to_vec();
    for &b in body {
        sum ^= b;
    }
    bytes.push(sum);
    for b in bytes {
        if matches!(b, 0x02 | 0x03 | 0x06 | 0x15 | 0x1B) {
            out.push(0x1B);
            out.push(b + 0x80);
        } else {
            out.push(b);
        }
    }
    out.push(0x03);
    out
}

async fn simulated_london() -> (u16, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let log = received.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let mut body = Vec::new();
        let mut escape = false;
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            log.lock().unwrap().extend_from_slice(&buf[..n]);
            let mut replies = Vec::new();
            for &b in &buf[..n] {
                match b {
                    0x02 => body.clear(),
                    0x03 => {
                        body.pop(); // checksum
                        if body.first() == Some(&0x89) {
                            // SET with the subscribed address and -10 dB.
                            let mut set = vec![0x88];
                            set.extend_from_slice(&body[1..9]);
                            set.extend_from_slice(&(-100_000i32).to_be_bytes());
                            replies.extend(frame(&set));
                        }
                        body.clear();
                    }
                    0x1B => escape = true,
                    b => {
                        body.push(if escape { b - 0x80 } else { b });
                        escape = false;
                    }
                }
            }
            if !replies.is_empty() {
                stream.write_all(&replies).await.unwrap();
            }
        }
    });
    (port, received)
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
async fn london_end_to_end() {
    let (port, received) = simulated_london().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "bss-london".into(),
            model: "blu".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"subscriptions": "0x010F/3/0x000100/0:gain_db"})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["nodes"]["271"]["3"]["256"]["0"]["value"] == -10.0)
    })
    .await;

    let outcome = core
        .execute(
            id,
            "get_sv",
            params(json!({"node": 271, "object": 256, "state_variable": 1})),
        )
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!(-100_000)
        })
    );

    let outcome = core
        .execute(
            id,
            "set_sv",
            params(json!({"node": 271, "object": 256, "state_variable": 0, "value": 0.0, "encoding": "gain_db"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    // The Interface Kit's own example: SET 0 dB on 0x010F/3/0x000100/0.
    let set = [
        0x02, 0x88, 0x01, 0x0F, 0x1B, 0x83, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x84, 0x03,
    ];
    tokio::time::timeout(Duration::from_secs(2), async {
        while !received
            .lock()
            .unwrap()
            .windows(set.len())
            .any(|w| w == set)
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the set reaches the device");
    core.close(id).await;
}
