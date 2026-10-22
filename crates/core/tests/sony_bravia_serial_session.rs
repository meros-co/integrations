//! A BRAVIA display's serial control simulated behind a serial-to-Ethernet
//! converter on loopback TCP, driven through the public API.
//!
//! The simulated display checks each frame's checksum as the knowledge
//! center's Serial Control page gives it, keeps power, input, volume and
//! muting, answers controls with 0x70 and a status, and queries with 0x70,
//! 0x00, a size, the data and a checksum; it cancels (0x03) everything but
//! power while off.

#![cfg(feature = "sony-bravia-serial")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Default)]
struct Display {
    on: bool,
    input: [u8; 2],
    volume: u8,
    muting: u8,
    received: Vec<Vec<u8>>,
}

fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

fn status(s: u8) -> Vec<u8> {
    vec![0x70, s, sum(&[0x70, s])]
}

fn data(d: &[u8]) -> Vec<u8> {
    let mut a = vec![0x70, 0x00, d.len() as u8 + 1];
    a.extend_from_slice(d);
    a.push(sum(&a));
    a
}

impl Display {
    fn answer(&mut self, frame: &[u8]) -> Vec<u8> {
        self.received.push(frame.to_vec());
        let function = frame[2];
        if frame[0] == 0x83 {
            return match (function, self.on) {
                (0x00, _) => data(&[u8::from(self.on)]),
                (_, false) => status(0x03),
                (0x02, true) => data(&self.input),
                (0x05, true) => data(&[self.volume]),
                (0x06, true) => data(&[self.muting]),
                _ => status(0x03),
            };
        }
        let d = &frame[4..frame.len() - 1];
        match (function, self.on) {
            (0x00, _) => self.on = d[0] == 1,
            (_, false) => return status(0x03),
            (0x02, true) => self.input = [d[0], d[1]],
            (0x05, true) if d[0] == 0x01 => {
                if d[1] > 100 {
                    return status(0x01);
                }
                self.volume = d[1];
            }
            (0x06, true) if d[0] == 0x01 => self.muting = d[1],
            _ => {}
        }
        status(0x00)
    }
}

async fn simulate() -> (u16, Arc<Mutex<Display>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let display = Arc::new(Mutex::new(Display {
        on: true,
        input: [0x04, 0x01],
        volume: 12,
        ..Default::default()
    }));
    let d = display.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let d = d.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 256];
                loop {
                    let n = stream.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    loop {
                        let len = match buf.first() {
                            Some(0x83) => 6,
                            Some(0x8C) if buf.len() >= 4 => 4 + buf[3] as usize,
                            _ => break,
                        };
                        if buf.len() < len {
                            break;
                        }
                        let frame: Vec<u8> = buf.drain(..len).collect();
                        assert_eq!(sum(&frame[..len - 1]), frame[len - 1], "checksum");
                        let answer = d.lock().unwrap().answer(&frame);
                        stream.write_all(&answer).await.unwrap();
                    }
                }
            });
        }
    });
    (port, display)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "sony-bravia-serial".into(),
        model: "bz".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"link": "tcp"})),
        monitor,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn state_is_read_and_commands_are_answered() {
    let (port, _display) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["power"] == true && s["volume"] == 12 && s["input"]["kind"] == "hdmi" {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s");
    assert_eq!(
        core.execute(id, "set_volume", params(json!({"volume": 30})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "get_volume", params(json!({}))).await,
        Ok(Outcome::Value { value: json!(30) })
    );
    assert!(matches!(
        core.execute(id, "set_volume", params(json!({"volume": 120})))
            .await,
        Err(CommandError::DeviceError { .. })
    ));
    assert_eq!(
        core.execute(
            id,
            "set_input",
            params(json!({"kind": "hdmi", "number": 3}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    // The input is read back after the control.
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().state["input"]["number"] != json!(3) {
            core.next_events(64).await;
        }
    })
    .await
    .expect("the input read back");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_power_is_asked() {
    let (port, display) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "set_power", params(json!({"power_on": false})))
            .await,
        Ok(Outcome::Ack)
    );
    // Off, the display cancels anything else.
    assert!(matches!(
        core.execute(id, "set_mute", params(json!({"muted": true})))
            .await,
        Err(CommandError::DeviceError { .. })
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let received = display.lock().unwrap().received.clone();
    assert!(
        received.iter().all(|f| f[2] == 0x00 || f[2] == 0x06),
        "{received:?}"
    );
    core.close(id).await;
}
