//! An HDX projector simulated on real TCP, speaking the Barco Projection
//! Protocol, driven through the public API. The simulator frames and checks
//! messages as the HDX catalog R5905447 §2.1 does (0xFE, address, command
//! bytes, checksum, 0xFF, with 0x80, 0xFE and 0xFF escaped), acknowledges
//! what it knows with 0x00 0x06 and answers the reads it knows with the
//! request's bytes and the value; anything else it refuses with 0x00 0x15.
//! The serial link is the same module on a serial port, tested in the
//! module's own tests.

#![cfg(feature = "barco-projection-protocol")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Core, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(body: &[u8]) -> Vec<u8> {
    let sum = body.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    let mut out = vec![0xFE];
    for &b in [&[0u8][..], body, &[sum]].concat().iter() {
        match b {
            0x80 => out.extend([0x80, 0x00]),
            0xFE => out.extend([0x80, 0x7E]),
            0xFF => out.extend([0x80, 0x7F]),
            b => out.push(b),
        }
    }
    out.push(0xFF);
    out
}

fn unframe(raw: &[u8]) -> Vec<u8> {
    let mut plain = Vec::new();
    let mut it = raw.iter();
    while let Some(&b) = it.next() {
        if b == 0x80 {
            plain.push(match it.next() {
                Some(0x00) => 0x80,
                Some(0x7E) => 0xFE,
                Some(0x7F) => 0xFF,
                other => panic!("bad escape {other:?}"),
            });
        } else {
            plain.push(b);
        }
    }
    let check = plain.pop().unwrap();
    assert_eq!(
        plain.iter().fold(0u8, |a, &b| a.wrapping_add(b)),
        check,
        "checksum"
    );
    assert_eq!(plain.remove(0), 0, "address 0 over Ethernet");
    plain
}

struct Projector {
    brightness: Mutex<u8>,
    source: Mutex<u8>,
    /// Every request received, command bytes first.
    received: Mutex<Vec<Vec<u8>>>,
}

impl Projector {
    fn answer(&self, msg: &[u8]) -> Vec<Vec<u8>> {
        const ACK: [u8; 2] = [0x00, 0x06];
        const NACK: [u8; 2] = [0x00, 0x15];
        match msg {
            [0x67] => vec![ACK.to_vec(), vec![0x67, 0x41]],
            [0x32] => vec![ACK.to_vec(), vec![0x32, *self.source.lock().unwrap()]],
            [0x31, n] => {
                *self.source.lock().unwrap() = *n;
                vec![ACK.to_vec()]
            }
            [0x21, 0x02] => vec![
                ACK.to_vec(),
                vec![0x21, 0x02, *self.brightness.lock().unwrap()],
            ],
            [0x20, 0x02, v] => {
                *self.brightness.lock().unwrap() = *v;
                vec![ACK.to_vec()]
            }
            _ => vec![NACK.to_vec()],
        }
    }

    async fn serve(self: Arc<Self>, mut stream: tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            let n = stream.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while let Some(end) = buf.iter().position(|&b| b == 0xFF) {
                assert_eq!(buf[0], 0xFE, "start byte");
                let msg = unframe(&buf[1..end]);
                buf.drain(..=end);
                self.received.lock().unwrap().push(msg.clone());
                for reply in self.answer(&msg) {
                    stream.write_all(&frame(&reply)).await.unwrap();
                }
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector {
        brightness: Mutex::new(0x80),
        source: Mutex::new(1),
        received: Mutex::new(Vec::new()),
    });
    let p = projector.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(p.clone().serve(stream));
        }
    });
    (port, projector)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "barco-projection-protocol".into(),
        model: "hdx-w14".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({})),
        monitor,
    })
    .unwrap()
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn state_is_polled_and_settings_are_changed_and_read_back() {
    let (port, _projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| {
        s["status"]["projector_on"] == true
            && s["status"]["lamp_on"] == true
            && s["source"]["number"] == 1
    })
    .await;
    wait_for_state(&core, id, |s| s["picture"]["brightness"] == 0x80).await;

    assert_eq!(
        core.execute(id, "set_brightness", params(json!({"value": 77})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["picture"]["brightness"] == 77).await;
    assert_eq!(
        core.execute(id, "set_source", params(json!({"source": 3})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "get_source", params(json!({}))).await,
        Ok(Outcome::Value { value: json!(3) })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    // A command the projector refuses fails with its NACK.
    assert!(core
        .execute(id, "set_contrast", params(json!({"value": 10})))
        .await
        .is_err());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_liveness_check_is_sent() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "get_brightness", params(json!({}))).await,
        Ok(Outcome::Value { value: json!(0x80) })
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut received = projector.received.lock().unwrap().clone();
    received.sort();
    // The command, and the status as the liveness check: nothing else.
    assert_eq!(received, [vec![0x21, 0x02], vec![0x67]], "{received:?}");
    core.close(id).await;
}
