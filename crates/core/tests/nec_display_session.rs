//! An NEC large-format display simulated on real TCP, driven through the
//! public API. The simulator frames and checks messages as NEC's External
//! Control documents do (§4: SOH, '0', the destination, '0', the type, a hex
//! length, STX, the hex body, ETX, the check code, CR), answers gets and sets
//! of operation codes (§5) and the power commands, and keeps what was set.

#![cfg(feature = "nec-display")]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn reply(kind: u8, body: &str) -> Vec<u8> {
    let mut out = vec![0x01, b'0', b'0', b'A', kind];
    out.extend(format!("{:02X}", body.len() + 2).as_bytes());
    out.push(0x02);
    out.extend(body.as_bytes());
    out.push(0x03);
    let bcc = out[1..].iter().fold(0, |a, b| a ^ b);
    out.push(bcc);
    out.push(0x0D);
    out
}

struct Display {
    /// Operation code (page and code as hex text) to value.
    values: Mutex<HashMap<String, u16>>,
    power: Mutex<u16>,
    received: Mutex<Vec<(u8, String)>>,
}

impl Display {
    fn answer(&self, kind: u8, body: &str) -> Vec<u8> {
        let mut values = self.values.lock().unwrap();
        match kind {
            b'C' => match values.get(body) {
                Some(v) => reply(b'D', &format!("00{body}000064{v:04X}")),
                None => reply(b'D', &format!("01{body}0000000000")),
            },
            b'E' => {
                let (op, value) = body.split_at(4);
                let v = u16::from_str_radix(value, 16).unwrap();
                values.insert(op.to_string(), v);
                reply(b'F', &format!("00{op}000064{v:04X}"))
            }
            b'A' if body == "01D6" => reply(
                b'B',
                &format!("0200D6000004{:04X}", *self.power.lock().unwrap()),
            ),
            b'A' if body.starts_with("C203D6") => {
                *self.power.lock().unwrap() = u16::from_str_radix(&body[6..], 16).unwrap();
                reply(b'B', &format!("00{body}"))
            }
            b'A' if body == "C217" => reply(b'B', "C3174D373531"),
            _ => reply(b'B', "BE"),
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
            while buf.len() >= 7 {
                let len =
                    usize::from_str_radix(std::str::from_utf8(&buf[5..7]).unwrap(), 16).unwrap();
                let total = 7 + len + 2;
                if buf.len() < total {
                    break;
                }
                let message: Vec<u8> = buf.drain(..total).collect();
                assert_eq!(message[0], 0x01, "SOH");
                assert_eq!(message[2], b'A', "monitor ID 1");
                let etx = 7 + len - 1;
                let bcc = message[1..=etx].iter().fold(0, |a, b| a ^ b);
                assert_eq!(message[etx + 1], bcc, "check code");
                assert_eq!(message[etx + 2], 0x0D, "delimiter");
                let body = String::from_utf8(message[8..etx].to_vec()).unwrap();
                self.received
                    .lock()
                    .unwrap()
                    .push((message[4], body.clone()));
                stream
                    .write_all(&self.answer(message[4], &body))
                    .await
                    .unwrap();
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Display>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let values = [("0060", 0x11), ("0062", 30), ("008D", 2), ("0010", 50)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let display = Arc::new(Display {
        values: Mutex::new(values),
        power: Mutex::new(1),
        received: Mutex::new(Vec::new()),
    });
    let d = display.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(d.clone().serve(stream));
        }
    });
    (port, display)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "nec-display".into(),
        model: "g4".into(),
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
async fn state_is_read_kept_current_and_changed() {
    let (port, _display) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| {
        s["power"] == "on"
            && s["input"] == "hdmi1"
            && s["audio"]["volume"] == 30
            && s["audio"]["muted"] == false
    })
    .await;
    assert_eq!(
        core.execute(id, "set_backlight", params(json!({"value": 70})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["picture"]["backlight"] == 70).await;
    assert_eq!(
        core.execute(id, "get_model", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("M751")
        })
    );
    assert_eq!(
        core.execute(id, "get_vcp", params(json!({"page": 0, "code": 0x62})))
            .await,
        Ok(Outcome::Value {
            value: json!({"max": 100, "value": 30})
        })
    );
    // An operation code the display does not support (result 01).
    assert!(core
        .execute(id, "get_tint", params(json!({})))
        .await
        .is_err());
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_liveness_check_is_sent() {
    let (port, display) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "set_mute", params(json!({"value": true})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut received = display.received.lock().unwrap().clone();
    received.sort();
    // The power status that shows the display is there, the set and its
    // read-back: nothing else.
    assert_eq!(
        received,
        [
            (b'A', "01D6".to_string()),
            (b'C', "008D".to_string()),
            (b'E', "008D0001".to_string())
        ],
        "{received:?}"
    );
    core.close(id).await;
}
