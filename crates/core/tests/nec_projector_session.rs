//! An NEC projector simulated on real TCP, driven through the public API.
//!
//! The simulator reads NEC's binary frames, checks their checksums, and
//! answers as the Projector Control Command Reference Manual describes:
//! basic information, model name and serial number, the error status,
//! 060-1 gains, power and input commands, and the error pair 00h 01h (not
//! supported) for anything else.

#![cfg(feature = "nec-projector")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn checksum(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |a, x| a.wrapping_add(*x))
}

fn reply(ok: bool, class: u8, code: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![
        if ok { 0x20 } else { 0xA0 } | class,
        code,
        0x01,
        0x2E,
        data.len() as u8,
    ];
    out.extend_from_slice(data);
    out.push(checksum(&out));
    out
}

struct Projector {
    powered: Mutex<bool>,
    /// Selection signal type 1 and 2.
    input: Mutex<(u8, u8)>,
    brightness: Mutex<i16>,
    /// Every command received, as (class, code, data).
    received: Mutex<Vec<(u8, u8, Vec<u8>)>>,
}

impl Projector {
    fn answer(&self, class: u8, code: u8, data: &[u8]) -> Vec<u8> {
        match (class, code, data) {
            (0x00, 0xBF, [0x02]) => {
                let (n, t) = *self.input.lock().unwrap();
                let op = if *self.powered.lock().unwrap() {
                    0x04
                } else {
                    0x00
                };
                let mut d = vec![0x02, op, 0x00, n, t, 0xFF, 0x00, 0x00, 0x00, 0x00];
                d.resize(16, 0);
                reply(true, 0x00, 0xBF, &d)
            }
            (0x00, 0xBF, [0x01, 0x06]) => {
                let mut d = vec![0x01, 0x06];
                d.extend_from_slice(b"7Y00123AB");
                d.resize(18, 0);
                reply(true, 0x00, 0xBF, &d)
            }
            (0x00, 0x85, [0x04]) => {
                let mut d = b"NP-PA1004UL".to_vec();
                d.resize(32, 0);
                reply(true, 0x00, 0x85, &d)
            }
            (0x00, 0x88, []) => {
                let mut d = vec![0u8; 12];
                d[0] = 0x01; // cover error
                reply(true, 0x00, 0x88, &d)
            }
            (0x03, 0x05, [0x00, 0x00, 0x00]) => {
                let v = self.brightness.lock().unwrap().to_le_bytes();
                let d = [
                    0x02, 100, 0, 0x9C, 0xFF, 0, 0, v[0], v[1], 1, 0, 1, 0, 0x01, 0, 0,
                ];
                reply(true, 0x03, 0x05, &d)
            }
            (0x02, 0x00, []) => {
                *self.powered.lock().unwrap() = true;
                reply(true, 0x02, 0x00, &[])
            }
            (0x02, 0x01, []) => {
                *self.powered.lock().unwrap() = false;
                reply(true, 0x02, 0x01, &[])
            }
            (0x02, 0x03, [0x01, terminal]) => match terminal {
                0xA1 => {
                    *self.input.lock().unwrap() = (0x01, 0x21);
                    reply(true, 0x02, 0x03, &[0x00])
                }
                0xA2 => {
                    *self.input.lock().unwrap() = (0x02, 0x21);
                    reply(true, 0x02, 0x03, &[0x00])
                }
                // 01h 01h: the specified input terminal is invalid.
                _ => reply(false, 0x02, 0x03, &[0x01, 0x01]),
            },
            (0x03, 0x10, [0x00, 0xFF, 0x00, lo, hi]) => {
                *self.brightness.lock().unwrap() = i16::from_le_bytes([*lo, *hi]);
                reply(true, 0x03, 0x10, &[0x00, 0x00])
            }
            _ => reply(false, class, code, &[0x00, 0x01]),
        }
    }

    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        let mut pending = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            while pending.len() >= 5 && pending.len() > 5 + pending[4] as usize {
                let total = 5 + pending[4] as usize + 1;
                let frame: Vec<u8> = pending.drain(..total).collect();
                assert_eq!(
                    checksum(&frame[..total - 1]),
                    frame[total - 1],
                    "{frame:02X?}"
                );
                assert_eq!(&frame[2..4], &[0, 0], "ID1 and ID2 are sent as 00h");
                let (class, code, data) = (frame[0], frame[1], frame[5..total - 1].to_vec());
                self.received
                    .lock()
                    .unwrap()
                    .push((class, code, data.clone()));
                let out = self.answer(class, code, &data);
                stream.write_all(&out).await.unwrap();
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector {
        powered: Mutex::new(false),
        input: Mutex::new((0x01, 0x21)),
        brightness: Mutex::new(0),
        received: Mutex::default(),
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

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn status_is_polled_and_commands_are_answered() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "nec-projector".into(),
            model: "np-pa1004ul".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    // Identity and status from the first poll.
    wait_for_state(&core, id, |s| {
        s["info"]["model_name"] == "NP-PA1004UL"
            && s["info"]["serial_number"] == "7Y00123AB"
            && s["power"] == "standby"
            && s["input"]["type"] == "hdmi"
            && s["errors"]["cover"] == true
            && s["picture"]["brightness"]["value"] == 0
    })
    .await;

    // Power on, then the status read after it.
    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["power"] == "on").await;

    // HDMI2 by name, sent as A2h, and read back as HDMI number 2.
    assert_eq!(
        core.execute(id, "set_input", params(json!({"input": "hdmi2"})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["input"]["number"] == 2).await;

    // A picture adjustment is read back with 060-1.
    assert_eq!(
        core.execute(
            id,
            "set_picture",
            params(json!({"item": "brightness", "value": -12}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["picture"]["brightness"]["value"] == -12).await;

    // An error answer is a device error carrying its code pair.
    match core
        .execute(id, "set_input", params(json!({"input": "hdbaset"})))
        .await
    {
        Err(CommandError::DeviceError { code, .. }) => assert_eq!(code.as_deref(), Some("0101")),
        other => panic!("{other:?}"),
    }

    // A terminal the model lacks is refused without being sent.
    assert!(matches!(
        core.execute(id, "set_input", params(json!({"input": "sdi"})))
            .await,
        Err(CommandError::InvalidParams { .. })
    ));

    let received = projector.received.lock().unwrap().clone();
    assert!(received.contains(&(0x02, 0x03, vec![0x01, 0xA2])));
    assert!(received.contains(&(0x03, 0x10, vec![0x00, 0xFF, 0x00, 0xF4, 0xFF])));
    assert!(!received.iter().any(|(_, _, d)| d == &vec![0x01, 0xC4]));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_only_asks_nothing_but_the_liveness_request() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "nec-projector".into(),
            model: "np-pa1004ul".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: false,
        })
        .unwrap();
    wait_for_state(&core, id, |s| s["power"] == "standby").await;
    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": true})))
            .await,
        Ok(Outcome::Ack)
    );
    let received = projector.received.lock().unwrap().clone();
    assert!(
        received.iter().all(
            |(c, k, d)| (*c, *k, d.as_slice()) == (0x00, 0xBF, &[0x02][..])
                || (*c, *k) == (0x02, 0x00)
        ),
        "{received:02X?}"
    );
    core.close(id).await;
}
