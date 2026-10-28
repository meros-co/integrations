//! A Digital Projection INSIGHT 4K Laser simulated on real TCP, driven through
//! the public API. The simulator checks each request's length and checksum
//! as the Protocol Guide (115-482F, p.62) frames them, and answers with the
//! command bytes plus 20h, C0h, a length, data and a checksum, keeping what
//! was set.

#![cfg(feature = "digitalprojection-insight-laser")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn reply(command: [u8; 3], data: &[u8]) -> Vec<u8> {
    let mut out = vec![
        command[0] | 0x20,
        command[1],
        command[2],
        0xC0,
        data.len() as u8,
    ];
    out.extend_from_slice(data);
    out.push(out.iter().fold(0u8, |a, b| a.wrapping_add(*b)));
    out
}

#[derive(Default)]
struct Projector {
    power: Mutex<bool>,
    light_power: Mutex<u8>,
    douser_closed: Mutex<bool>,
    title: Mutex<u8>,
    received: Mutex<Vec<Vec<u8>>>,
}

impl Projector {
    fn answer(&self, command: [u8; 3], data: &[u8]) -> Vec<u8> {
        match (command, data) {
            ([0x02, 0x00, 0x00], _) => {
                *self.power.lock().unwrap() = true;
                reply(command, &[])
            }
            ([0x02, 0x01, 0x00], _) => {
                *self.power.lock().unwrap() = false;
                reply(command, &[])
            }
            ([0x03, 0x10, 0x00], d) => {
                *self.light_power.lock().unwrap() = d[3];
                reply(command, &[0x00, 0x00])
            }
            ([0x03, 0x05, 0x00], _) => {
                let level = *self.light_power.lock().unwrap();
                let mut d = vec![0x02, 0x64, 0x00, 0x1E, 0x00, 0x00, 0x00, level];
                d.extend([0x00, 0x07, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xFF]);
                reply(command, &d)
            }
            ([0x02, 0x16, 0x00], _) | ([0x02, 0x17, 0x00], _) => {
                *self.douser_closed.lock().unwrap() = command[1] == 0x16;
                reply(command, &[])
            }
            ([0x02, 0x03, 0x00], d) => {
                *self.title.lock().unwrap() = d[1];
                reply(command, &[0x00])
            }
            ([0x00, 0x85, 0x00], [0x01]) => {
                let on = *self.power.lock().unwrap();
                let mut d = vec![0u8; 16];
                d[2] = u8::from(on);
                d[5] = if on { 0x0C } else { 0x00 };
                reply(command, &d)
            }
            ([0x00, 0x85, 0x00], [0x03]) => {
                let mut d = vec![0u8; 16];
                d[0] = if *self.douser_closed.lock().unwrap() {
                    0x81
                } else {
                    0x00
                };
                reply(command, &d)
            }
            ([0x00, 0x85, 0x00], [0x02]) => {
                let mut d = vec![0u8; 16];
                d[1] = *self.title.lock().unwrap();
                reply(command, &d)
            }
            ([0x03, 0x2F, 0x00], [0x1E]) => reply(
                command,
                &[
                    0x1E, 0x42, 0x00, 0x20, 0x4E, 0x64, 0x47, 0x00, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
            _ => reply(command, &[0x00]),
        }
    }

    async fn serve(self: Arc<Self>, mut stream: tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            let n = stream.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while buf.len() >= 6 {
                let len = usize::from(u16::from_be_bytes([buf[3], buf[4]]));
                let size = 6 + len;
                if buf.len() < size {
                    break;
                }
                let message: Vec<u8> = buf.drain(..size).collect();
                let sum = message[..size - 1]
                    .iter()
                    .fold(0u8, |a, b| a.wrapping_add(*b));
                assert_eq!(sum, message[size - 1], "checksum");
                self.received.lock().unwrap().push(message.clone());
                let command = [message[0], message[1], message[2]];
                let answer = self.answer(command, &message[5..size - 1]);
                stream.write_all(&answer).await.unwrap();
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector::default());
    *projector.light_power.lock().unwrap() = 80;
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
        device: "digitalprojection-insight-laser".into(),
        model: "insight-4k-laser".into(),
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
async fn state_is_read_and_changes_are_read_back() {
    let (port, _projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| {
        s["power"] == false
            && s["mode"] == "standby"
            && s["light_power"] == 80
            && s["light_status"]["hours"] == 66
            && s["douser_closed"] == false
            && s["title"] == 1
    })
    .await;
    assert_eq!(
        core.execute(id, "power_on", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["power"] == true && s["mode"] == "running_light_off"
    })
    .await;
    assert_eq!(
        core.execute(id, "set_light_power", params(json!({"percent": 55})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["light_power"] == 55).await;
    assert_eq!(
        core.execute(id, "close_douser", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["douser_closed"] == true).await;
    assert_eq!(
        core.execute(id, "set_title", params(json!({"title": 12})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["title"] == 12).await;
    assert_eq!(
        core.execute(id, "get_title", params(json!({}))).await,
        Ok(Outcome::Value { value: json!(12) })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_running_status_is_read() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "zoom", params(json!({"direction": "in"})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut received = projector.received.lock().unwrap().clone();
    received.sort();
    // The running status that shows the projector is there, and the command.
    assert_eq!(
        received,
        [
            vec![0x00, 0x85, 0x00, 0x00, 0x01, 0x01, 0x87],
            vec![0x02, 0x18, 0x00, 0x00, 0x02, 0x00, 0x7F, 0x9B],
        ],
        "{received:?}"
    );
    core.close(id).await;
}
