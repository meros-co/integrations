//! A DHD core speaking ECP, simulated on real TCP, driven through the public
//! API.
//!
//! The simulator reads 16-byte blocks as DHD's "ECP Communication" page lays
//! them out (length, unused byte, ID, eight data bytes, two unused bytes),
//! keeps logic states, fader levels and 32-bit crosspoints, answers their
//! requests, and when crosspoint notification is on sends every route it
//! makes. Like the device, it acknowledges nothing.

#![cfg(feature = "dhd-ecp")]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Connection, Core, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn block(id: u32, data: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 16];
    b[0] = data.len() as u8;
    b[2..6].copy_from_slice(&id.to_be_bytes());
    b[6..6 + data.len()].copy_from_slice(data);
    b
}

#[derive(Default)]
struct Core52 {
    logics: Mutex<HashMap<[u8; 2], u8>>,
    levels: Mutex<HashMap<[u8; 2], [u8; 2]>>,
    routes: Mutex<HashMap<[u8; 4], [u8; 4]>>,
    notify: Mutex<bool>,
    /// Every block received, as (ID, data).
    received: Mutex<Vec<(u32, Vec<u8>)>>,
}

impl Core52 {
    fn answer(&self, id: u32, d: &[u8]) -> Vec<Vec<u8>> {
        match (id, d.len()) {
            (0x110E0000, 3) => {
                self.logics.lock().unwrap().insert([d[0], d[1]], d[2]);
                vec![]
            }
            (0x110E0000, 2) => {
                let v = *self.logics.lock().unwrap().get(&[d[0], d[1]]).unwrap_or(&0);
                vec![block(id, &[d[0], d[1], v])]
            }
            (0x11030000, 4) | (0x11030000, 7) => {
                self.levels
                    .lock()
                    .unwrap()
                    .insert([d[0], d[1]], [d[2], d[3]]);
                vec![]
            }
            (0x11030000, 2) => {
                let l = *self
                    .levels
                    .lock()
                    .unwrap()
                    .get(&[d[0], d[1]])
                    .unwrap_or(&[0x80, 0]);
                vec![block(id, &[d[0], d[1], l[0], l[1]])]
            }
            (0x01F50000, 1) => {
                *self.notify.lock().unwrap() = d[0] == 1;
                vec![]
            }
            (0x01F50000, 8) => {
                let (o, i) = ([d[0], d[1], d[2], d[3]], [d[4], d[5], d[6], d[7]]);
                self.routes.lock().unwrap().insert(o, i);
                if *self.notify.lock().unwrap() {
                    vec![block(id, d)]
                } else {
                    vec![]
                }
            }
            (0x01F50000, 5) => {
                let o = [d[0], d[1], d[2], d[3]];
                let i = *self.routes.lock().unwrap().get(&o).unwrap_or(&[0; 4]);
                vec![block(id, &[&o[..], &i].concat())]
            }
            _ => vec![],
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
            while buf.len() >= 16 {
                let b: Vec<u8> = buf.drain(..16).collect();
                assert_eq!(b[1], 0, "unused byte");
                assert_eq!(&b[14..], &[0, 0], "unused bytes");
                let len = usize::from(b[0]);
                assert!(len <= 8, "length {len}");
                assert!(b[6 + len..14].iter().all(|&x| x == 0), "unused data bytes");
                let id = u32::from_be_bytes([b[2], b[3], b[4], b[5]]);
                let data = b[6..6 + len].to_vec();
                self.received.lock().unwrap().push((id, data.clone()));
                for reply in self.answer(id, &data) {
                    stream.write_all(&reply).await.unwrap();
                }
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Core52>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let sim = Arc::new(Core52::default());
    let s = sim.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(s.clone().serve(stream));
        }
    });
    (port, sim)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "dhd-ecp".into(),
        model: "52-xc2".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"read_faders": 1})),
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

async fn connected(core: &Core, id: meros_integrations::DeviceId) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.snapshot(id).unwrap().connection == Connection::Connected {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("connected within 10 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_read_back_and_notifications_keep_state() {
    let (port, sim) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    // The level read on connecting (read_faders 1) arrives as mute level.
    wait_for_state(&core, id, |s| {
        s["mixers"]["1"]["faders"]["1"]["level"] == json!(-327.68)
    })
    .await;

    assert_eq!(
        core.execute(
            id,
            "set_fader_level",
            params(json!({"mixer": 1, "fader": 1, "level": -6.5}))
        )
        .await,
        Ok(Outcome::Value { value: json!(-6.5) })
    );
    assert_eq!(
        core.execute(
            id,
            "set_logic",
            params(json!({"logic": 719, "value": true}))
        )
        .await,
        Ok(Outcome::Value { value: json!(true) })
    );
    assert_eq!(
        core.execute(
            id,
            "set_route",
            params(json!({"output": "40080803", "input": "40010002"}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!("40010002")
        })
    );
    wait_for_state(&core, id, |s| {
        s["routes"]["40080803"] == "40010002" && s["logics"]["719"] == true
    })
    .await;
    // The crosspoint notification was switched on when monitoring started.
    assert!(sim
        .received
        .lock()
        .unwrap()
        .contains(&(0x01F50000, vec![1])));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_nothing_is_switched_on_or_read() {
    let (port, sim) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    connected(&core, id).await;
    assert_eq!(
        core.execute(id, "get_logic", params(json!({"logic": 5})))
            .await,
        Ok(Outcome::Value {
            value: json!(false)
        })
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let received = sim.received.lock().unwrap().clone();
    assert_eq!(received, vec![(0x110E0000, vec![0, 5])]);
    core.close(id).await;
}
