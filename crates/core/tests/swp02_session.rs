//! An SW-P-02 router simulated on real TCP (the `tcp` link), driven through
//! the public API. The simulator frames and checks messages as SW-P-88 Issue
//! 1 §3.1 does (FFh, command, data, 7-bit two's complement checksum) and
//! answers INTERROGATE with TALLY, CONNECT with CONNECTED, CONNECT ON GO with
//! its acknowledge and GO with GO DONE ACKNOWLEDGE. The serial link is the
//! same module on a serial port, tested in the module's own tests.

#![cfg(feature = "probel-swp02")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(body: &[u8]) -> Vec<u8> {
    let sum = body.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    [&[0xFF][..], body, &[sum.wrapping_neg() & 0x7F]].concat()
}

fn destination(m: &[u8]) -> usize {
    (usize::from((m[1] >> 4) & 7) << 7) | usize::from(m[2])
}

#[derive(Default)]
struct Router {
    /// Source routed to each destination, wire numbering.
    table: Mutex<Vec<u16>>,
    /// Stored connect-on-go routes.
    salvo: Mutex<Vec<(usize, u16)>>,
    /// Every message received, command byte first.
    received: Mutex<Vec<Vec<u8>>>,
}

impl Router {
    fn answer(&self, msg: &[u8]) -> Vec<Vec<u8>> {
        let mut table = self.table.lock().unwrap();
        match msg[0] {
            1 => {
                let s = table.get(destination(msg)).copied().unwrap_or(1023);
                vec![vec![
                    3,
                    msg[1] | ((s >> 7) & 7) as u8,
                    msg[2],
                    (s & 0x7F) as u8,
                ]]
            }
            2 => {
                table[destination(msg)] = (u16::from(msg[1] & 7) << 7) | u16::from(msg[3]);
                vec![vec![4, msg[1], msg[2], msg[3]]]
            }
            5 => {
                let s = (u16::from(msg[1] & 7) << 7) | u16::from(msg[3]);
                self.salvo.lock().unwrap().push((destination(msg), s));
                vec![vec![12, msg[1], msg[2], msg[3]]]
            }
            6 => {
                let mut salvo = self.salvo.lock().unwrap();
                let code = if salvo.is_empty() { 2 } else { msg[1] };
                if msg[1] == 0 {
                    for (d, s) in salvo.drain(..) {
                        table[d] = s;
                    }
                }
                salvo.clear();
                vec![vec![13, code]]
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
            loop {
                let len = match buf.get(1) {
                    Some(1) => 2,
                    Some(2 | 5) => 3,
                    Some(6) => 1,
                    Some(other) => panic!("unexpected command {other}"),
                    None => break,
                };
                if buf.len() < len + 3 {
                    break;
                }
                assert_eq!(buf[0], 0xFF, "start of message");
                let msg: Vec<u8> = buf[1..len + 2].to_vec();
                assert_eq!(frame(&msg), buf[..len + 3], "checksum");
                buf.drain(..len + 3);
                self.received.lock().unwrap().push(msg.clone());
                for reply in self.answer(&msg) {
                    stream.write_all(&frame(&reply)).await.unwrap();
                }
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Router>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Arc::new(Router {
        table: Mutex::new((0..200).collect()),
        ..Default::default()
    });
    let r = router.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(r.clone().serve(stream));
        }
    });
    (port, router)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "probel-swp02".into(),
        model: "generic".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"link": "tcp", "destinations": 4})),
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
async fn routing_is_read_kept_current_and_changed() {
    let (port, _router) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| s["destinations"]["4"]["source"] == 4).await;

    assert_eq!(
        core.execute(
            id,
            "connect",
            params(json!({"destination": 2, "source": 150}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["destinations"]["2"]["source"] == 150).await;
    assert_eq!(
        core.execute(id, "interrogate", params(json!({"destination": 2})))
            .await,
        Ok(Outcome::Value { value: json!(150) })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());

    assert_eq!(
        core.execute(
            id,
            "salvo_add",
            params(json!({"destination": 3, "source": 9}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "salvo_fire", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("set")
        })
    );
    // A GO sends no CONNECTED: the destinations are read again.
    wait_for_state(&core, id, |s| s["destinations"]["3"]["source"] == 9).await;
    assert_eq!(
        core.execute(id, "salvo_clear", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("none")
        })
    );
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_liveness_check_is_sent() {
    let (port, router) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "interrogate", params(json!({"destination": 3})))
            .await,
        Ok(Outcome::Value { value: json!(3) })
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut received = router.received.lock().unwrap().clone();
    received.sort();
    // The command, and destination 1 as the liveness check: nothing else.
    assert_eq!(received, [vec![1, 0, 0], vec![1, 0, 2]], "{received:?}");
    core.close(id).await;
}
