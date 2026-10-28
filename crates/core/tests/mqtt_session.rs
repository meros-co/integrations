//! An MQTT 3.1.1 broker simulated on loopback, driven through the public
//! API. The simulator answers CONNECT with CONNACK, SUBSCRIBE with SUBACK
//! granting the QoS asked, a QoS 1 PUBLISH with PUBACK, and PINGREQ with
//! PINGRESP, as the OASIS standard lays them out, and delivers each message
//! published to the client back to it at QoS 0 when it matches a filter
//! (a filter ending in # matching everything below).

#![cfg(feature = "generic-mqtt")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, DeviceId, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn packet(kind: u8, flags: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![(kind << 4) | flags];
    let mut n = body.len();
    loop {
        let mut b = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            b |= 0x80;
        }
        out.push(b);
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(body);
    out
}

#[derive(Default)]
struct Broker {
    filters: Mutex<Vec<String>>,
    connects: Mutex<Vec<Vec<u8>>>,
}

fn matches(filter: &str, topic: &str) -> bool {
    match filter.strip_suffix('#') {
        Some(prefix) => topic.starts_with(prefix),
        None => filter == topic,
    }
}

impl Broker {
    fn answer(&self, head: u8, body: &[u8]) -> Vec<Vec<u8>> {
        let word = |i: usize| usize::from(u16::from_be_bytes([body[i], body[i + 1]]));
        match head >> 4 {
            1 => {
                self.connects.lock().unwrap().push(body.to_vec());
                vec![packet(2, 0, &[0, 0])]
            }
            8 => {
                let mut at = 2;
                let mut codes = Vec::new();
                while at < body.len() {
                    let n = word(at);
                    let filter = String::from_utf8(body[at + 2..at + 2 + n].to_vec()).unwrap();
                    codes.push(body[at + 2 + n]);
                    self.filters.lock().unwrap().push(filter);
                    at += 3 + n;
                }
                vec![packet(9, 0, &[&body[..2], &codes[..]].concat())]
            }
            3 => {
                let qos = (head >> 1) & 3;
                let n = word(0);
                let topic = String::from_utf8(body[2..2 + n].to_vec()).unwrap();
                let payload_at = 2 + n + if qos > 0 { 2 } else { 0 };
                let mut out = Vec::new();
                if qos == 1 {
                    out.push(packet(4, 0, &body[2 + n..4 + n]));
                }
                if self
                    .filters
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|f| matches(f, &topic))
                {
                    let mut fwd = body[..2 + n].to_vec();
                    fwd.extend_from_slice(&body[payload_at..]);
                    out.push(packet(3, 0, &fwd));
                }
                out
            }
            12 => vec![packet(13, 0, &[])],
            _ => vec![],
        }
    }

    async fn serve(self: Arc<Self>, mut stream: tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let n = stream.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            loop {
                let mut len = 0usize;
                let mut used = 0;
                let mut complete = false;
                for (i, &b) in buf.iter().skip(1).take(4).enumerate() {
                    len += usize::from(b & 0x7F) << (7 * i);
                    if b & 0x80 == 0 {
                        used = i + 1;
                        complete = true;
                        break;
                    }
                }
                if !complete || buf.len() < 1 + used + len {
                    break;
                }
                let head = buf[0];
                let body: Vec<u8> = buf[1 + used..1 + used + len].to_vec();
                buf.drain(..1 + used + len);
                for reply in self.answer(head, &body) {
                    stream.write_all(&reply).await.unwrap();
                }
            }
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn wait_for(core: &Core, device: DeviceId, check: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(device).unwrap().state;
            if check(&state) {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("state")
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribes_publishes_and_keeps_messages_through_the_public_api() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let broker = Arc::new(Broker::default());
    let b = broker.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(b.clone().serve(stream));
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "generic-mqtt".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"client_id": "meros-test", "subscriptions": "studio/#@1"})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, device, |s| {
        s["subscriptions"]["studio/#"]["qos"] == json!(1)
    })
    .await;
    assert_eq!(
        broker.connects.lock().unwrap()[0][10..],
        *b"\x00\x0ameros-test"
    );

    let outcome = core
        .execute(
            device,
            "publish",
            params(json!({"topic": "studio/cam1/tally", "payload": "program", "qos": 1})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    let state = wait_for(&core, device, |s| {
        s["topics"]["studio/cam1/tally"]["payload"] == json!("program")
    })
    .await;
    assert_eq!(state["topics"]["studio/cam1/tally"]["qos"], json!(0));

    let outcome = core
        .execute(
            device,
            "subscribe",
            params(json!({"filter": "other", "qos": 0})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Value { value: json!(0) }));
    core.close(device).await;
}
