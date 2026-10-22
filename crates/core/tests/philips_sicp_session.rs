//! A Philips display simulated on real TCP, driven through the public API.
//! The simulator frames and checks messages as SICP V2.03 §2.2.1 does
//! (length, monitor ID, group, data, XOR checksum), answers gets with a
//! report under the get's code and sets with ACK (§2.4), keeping what was set.

#![cfg(feature = "philips-sicp")]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(data: &[u8]) -> Vec<u8> {
    let mut out = vec![(data.len() + 4) as u8, 0x01, 0x00];
    out.extend_from_slice(data);
    out.push(out.iter().fold(0, |a, b| a ^ b));
    out
}

struct Display {
    /// Each get code's report data.
    values: Mutex<HashMap<u8, Vec<u8>>>,
    received: Mutex<Vec<Vec<u8>>>,
}

impl Display {
    fn answer(&self, data: &[u8]) -> Vec<u8> {
        let mut values = self.values.lock().unwrap();
        let set_to_get: &[(u8, u8)] = &[
            (0x18, 0x19),
            (0x44, 0x45),
            (0x47, 0x46),
            (0x32, 0x33),
            (0xAC, 0xAD),
        ];
        if let Some((_, get)) = set_to_get.iter().find(|(s, _)| *s == data[0]) {
            let mut stored = data[1..].to_vec();
            if data[0] == 0xAC {
                stored.truncate(2);
            }
            values.insert(*get, stored);
            return frame(&[0x00, 0x06]);
        }
        match values.get(&data[0]) {
            Some(v) => {
                let mut report = vec![data[0]];
                report.extend(v);
                frame(&report)
            }
            // Not available (NAV).
            None => frame(&[0x00, 0x18]),
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
            while let Some(&size) = buf.first() {
                let size = usize::from(size);
                if buf.len() < size {
                    break;
                }
                let message: Vec<u8> = buf.drain(..size).collect();
                assert_eq!(message.iter().fold(0u8, |a, b| a ^ b), 0, "checksum");
                assert_eq!(message[1], 0x01, "monitor ID");
                let data = message[3..size - 1].to_vec();
                self.received.lock().unwrap().push(data.clone());
                stream.write_all(&self.answer(&data)).await.unwrap();
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Display>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut values = HashMap::new();
    values.insert(0x19, vec![0x02]);
    values.insert(0xAD, vec![0x0D, 0x00]);
    values.insert(0x45, vec![30, 10]);
    values.insert(0x46, vec![0x00]);
    values.insert(0x33, vec![50, 50, 50, 5, 50, 50, 0x03]);
    values.insert(0xA1, b"65BDL4150D/00".to_vec());
    let display = Arc::new(Display {
        values: Mutex::new(values),
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
        device: "philips-sicp".into(),
        model: "android".into(),
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
        s["power"] == true && s["audio"]["volume"] == 30 && s["input"]["source"] == "hdmi"
    })
    .await;

    assert_eq!(
        core.execute(id, "set_volume", params(json!({"level": 45})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["audio"]["volume"] == 45 && s["audio"]["audio_out_volume"] == 10
    })
    .await;
    assert_eq!(
        core.execute(id, "set_sharpness", params(json!({"level": 8})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["picture"]["sharpness"] == 8 && s["picture"]["gamma"] == "2.2"
    })
    .await;
    assert_eq!(
        core.execute(
            id,
            "set_input",
            params(json!({"source": "displayport", "playlist": 0, "show_label": true}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["input"]["source"] == "displayport").await;
    assert_eq!(
        core.execute(id, "get_model", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("65BDL4150D/00")
        })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    // A command the display lacks is answered NAV.
    assert!(core
        .execute(id, "get_led_strip", params(json!({})))
        .await
        .is_err());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_liveness_check_is_sent() {
    let (port, display) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": false})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut received = display.received.lock().unwrap().clone();
    received.sort();
    // The power read that shows the display is there, the command, and its
    // read-back: nothing else.
    assert_eq!(
        received,
        [vec![0x18, 0x01], vec![0x19], vec![0x19]],
        "{received:?}"
    );
    core.close(id).await;
}
