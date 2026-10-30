//! A TimeMachines clock simulated on real UDP, driven through the public
//! API. The simulator answers the locator query A1 04 B2 with the 40-byte
//! clock status of the Locator Protocol API 2.3 (§1.1.1) and every control
//! request with 'A', keeping the mode the timer requests set.

#![cfg(feature = "timemachines-clock")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Core, OpenRequest, Outcome};
use tokio::net::UdpSocket;

#[derive(Default)]
struct Clock {
    /// Display mode byte (§1.1.1 byte 19).
    mode: Mutex<u8>,
    received: Mutex<Vec<Vec<u8>>>,
}

impl Clock {
    fn status(&self) -> Vec<u8> {
        let mut d = vec![0x01, 127, 0, 0, 1, 0x00, 0x1B, 0x2C, 0x01, 0x02, 0x03, 5, 4];
        d.extend([
            0x00,
            0x07,
            0,
            0,
            12,
            0,
            *self.mode.lock().unwrap(),
            0,
            0,
            0,
            0,
        ]);
        let mut name = b"Stage Left".to_vec();
        name.resize(16, 0);
        d.extend(name);
        d
    }

    fn answer(&self, data: &[u8]) -> Vec<u8> {
        match data[0] {
            0xA1 => return self.status(),
            0xA2 => *self.mode.lock().unwrap() = 0x01,
            0xA3 => {
                let mut m = self.mode.lock().unwrap();
                *m = (*m & !0x40) | if data[1] == 1 { 0x40 } else { 0 };
            }
            0xA8 => *self.mode.lock().unwrap() = 0x00,
            _ => {}
        }
        b"A".to_vec()
    }
}

async fn simulate() -> (u16, Arc<Clock>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let clock = Arc::new(Clock::default());
    let c = clock.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok((n, from)) = socket.recv_from(&mut buf).await {
            c.received.lock().unwrap().push(buf[..n].to_vec());
            let answer = c.answer(&buf[..n]);
            let _ = socket.send_to(&answer, from).await;
        }
    });
    (port, clock)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "timemachines-clock".into(),
        model: "poe-display".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"poll_ms": 250})),
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
async fn the_status_is_read_and_a_timer_is_started() {
    let (port, _clock) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| {
        s["display"]["mode"] == "time" && s["info"]["name"] == "Stage Left"
    })
    .await;
    assert_eq!(
        core.execute(id, "use_up_timer", params(json!({"format": "h_m_s"})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "set_up_timer_running", params(json!({"running": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["display"]["mode"] == "up_timer" && s["display"]["running"] == true
    })
    .await;
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_first_status_is_read() {
    let (port, clock) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "set_dimmer", params(json!({"digits": 50, "dots": 20})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(800)).await;
    let received = clock.received.lock().unwrap().clone();
    // The status read on opening, the command and its read-back; no polls.
    let queries = received.iter().filter(|r| r[0] == 0xA1).count();
    assert_eq!(queries, 2, "{received:?}");
    assert!(received.contains(&vec![0xB5, 50, 20]));
    core.close(id).await;
}
