//! An SW-P-08 router simulated on real TCP, driven through the public API.
//!
//! The simulator frames and checks messages as SW-P-08 Issue 30 §2.2 does
//! (DLE STX, data, byte count, two's complement checksum, DLE ETX, every 10h
//! doubled), acknowledges each good frame with DLE ACK, and answers: tally
//! dumps with word dumps (23), source names (106), destination names (107),
//! interrogates with TALLY (03), connects with a broadcast CONNECTED (04), and
//! the implementation request (98). It records whether the core acknowledges
//! the frames it sends.

#![cfg(feature = "probel-swp08")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const DLE: u8 = 0x10;

fn frame(data: &[u8]) -> Vec<u8> {
    let btc = data.len() as u8;
    let sum: u32 = data.iter().map(|&b| u32::from(b)).sum::<u32>() + u32::from(btc);
    let chk = (256 - (sum % 256)) as u8;
    let mut out = vec![DLE, 0x02];
    for &b in data.iter().chain([btc, chk].iter()) {
        out.push(b);
        if b == DLE {
            out.push(DLE);
        }
    }
    out.extend([DLE, 0x03]);
    out
}

#[derive(Default)]
struct Router {
    /// Source routed to each destination, wire numbering, matrix 0 level 0.
    table: Mutex<Vec<u16>>,
    /// Every message received (command byte first).
    received: Mutex<Vec<Vec<u8>>>,
    /// DLE ACKs the core sent back.
    acks: Mutex<usize>,
}

impl Router {
    fn answer(&self, msg: &[u8]) -> Vec<Vec<u8>> {
        let mut table = self.table.lock().unwrap();
        match msg[0] {
            // Tally dump request: one word dump of the whole table.
            21 => {
                let mut d = vec![23, msg[1], table.len() as u8, 0, 0];
                for s in table.iter() {
                    d.extend(s.to_be_bytes());
                }
                vec![d]
            }
            1 => {
                let dest = (usize::from((msg[2] >> 4) & 7) << 7) | usize::from(msg[3]);
                let s = table[dest];
                vec![vec![
                    3,
                    msg[1],
                    msg[2] | ((s >> 7) & 7) as u8,
                    msg[3],
                    (s & 0x7F) as u8,
                ]]
            }
            2 => {
                let dest = (usize::from((msg[2] >> 4) & 7) << 7) | usize::from(msg[3]);
                table[dest] = (u16::from(msg[2] & 7) << 7) | u16::from(msg[4]);
                vec![vec![4, msg[1], msg[2], msg[3], msg[4]]]
            }
            100 => vec![[&[106, msg[1], 1, 0, 0, 2][..], b"CAMERA 1VTR 2   "].concat()],
            102 => vec![[&[107, msg[1], 1, 0, 0, 1][..], b"PGM     "].concat()],
            97 => vec![vec![98, 2, 3, 3, 4, 1, 2, 21]],
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
            // Take every complete item off the front of the buffer.
            loop {
                if buf.len() >= 2 && buf[0] == DLE && buf[1] == 0x06 {
                    *self.acks.lock().unwrap() += 1;
                    buf.drain(..2);
                    continue;
                }
                if buf.len() < 2 || buf[0] != DLE || buf[1] != 0x02 {
                    break;
                }
                let mut content = Vec::new();
                let mut i = 2;
                let mut end = None;
                while i + 1 < buf.len() {
                    if buf[i] == DLE {
                        match buf[i + 1] {
                            DLE => content.push(DLE),
                            0x03 => {
                                end = Some(i + 2);
                                break;
                            }
                            other => panic!("unexpected DLE {other:02x} in a frame"),
                        }
                        i += 2;
                    } else {
                        content.push(buf[i]);
                        i += 1;
                    }
                }
                let Some(end) = end else { break };
                buf.drain(..end);
                let chk = content.pop().unwrap();
                let btc = content.pop().unwrap();
                let sum: u32 = content.iter().map(|&b| u32::from(b)).sum::<u32>()
                    + u32::from(btc)
                    + u32::from(chk);
                assert_eq!(usize::from(btc), content.len(), "byte count");
                assert_eq!(sum % 256, 0, "checksum");
                self.received.lock().unwrap().push(content.clone());
                stream.write_all(&[DLE, 0x06]).await.unwrap();
                for reply in self.answer(&content) {
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
        table: Mutex::new(vec![0, 1, 2, 3]),
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
        device: "probel-swp08".into(),
        model: "generic".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"name_length": "8", "read_protects": false})),
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
async fn routing_and_names_are_read_and_kept_current() {
    let (port, router) = simulate().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, true);

    wait_for_state(&core, id, |s| {
        let level = &s["matrices"]["1"]["levels"]["1"];
        level["destinations"]["4"]["source"] == 4
            && level["sources"]["2"]["name"] == "VTR 2"
            && s["matrices"]["1"]["destinations"]["1"]["name"] == "PGM"
            && s["implementation"]["received"] == json!([1, 2, 21])
    })
    .await;

    assert_eq!(
        core.execute(
            id,
            "connect",
            params(json!({"destination": 2, "source": 4}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["matrices"]["1"]["levels"]["1"]["destinations"]["2"]["source"] == 4
    })
    .await;
    assert_eq!(
        core.execute(id, "interrogate", params(json!({"destination": 2})))
            .await,
        Ok(Outcome::Value { value: json!(4) })
    );
    // Every frame the router sent was acknowledged.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(*router.acks.lock().unwrap() >= 6);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_nothing_is_read() {
    let (port, router) = simulate().await;
    let core = Core::new().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "interrogate", params(json!({"destination": 3})))
            .await,
        Ok(Outcome::Value { value: json!(3) })
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let received = router.received.lock().unwrap().clone();
    assert!(received.contains(&vec![8]), "{received:?}");
    assert!(
        received.iter().all(|m| m[0] == 8 || m[0] == 1),
        "{received:?}"
    );
    core.close(id).await;
}
