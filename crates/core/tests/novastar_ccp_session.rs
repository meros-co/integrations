//! A NovaStar controller's central control protocol simulated on real TCP,
//! driven through the public API.
//!
//! The simulator reads request frames, checks each checksum as the COEX
//! Central Control Protocol Instructions (V1.5.0, 3.1) define it, and answers
//! as its 3.1 D example does: the header `aa 55`, the request's register
//! address, no data, and a checksum over the whole answer. A register listed
//! in `refuse` is answered with ACK 1.

#![cfg(feature = "novastar-central-control")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Controller {
    /// Every request frame received, with a valid checksum.
    requests: Mutex<Vec<Vec<u8>>>,
    refuse: Vec<[u8; 4]>,
}

fn sum(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0x5555u32, |s, b| s + *b as u32) as u16
}

impl Controller {
    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            let Ok(n) = stream.read(&mut chunk).await else {
                return;
            };
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while buf.len() >= 20 {
                assert_eq!(&buf[..2], &[0x55, 0xaa], "a request starts 55 aa");
                let len = u16::from_le_bytes([buf[16], buf[17]]) as usize;
                let total = 18 + len + 2;
                if buf.len() < total {
                    break;
                }
                let frame: Vec<u8> = buf.drain(..total).collect();
                let got = u16::from_le_bytes([frame[total - 2], frame[total - 1]]);
                assert_eq!(got, sum(&frame[2..total - 2]), "request checksum");
                let register = [frame[12], frame[13], frame[14], frame[15]];
                self.requests.lock().unwrap().push(frame);
                let ack = u8::from(self.refuse.contains(&register));
                let mut answer = vec![
                    0xaa, 0x55, ack, 0x00, 0xff, 0xfe, 0x01, 0xff, 0xff, 0xff, 0x01, 0x00,
                ];
                answer.extend_from_slice(&register);
                answer.extend_from_slice(&[0x00, 0x00]);
                let s = sum(&answer);
                answer.extend_from_slice(&s.to_le_bytes());
                stream.write_all(&answer).await.unwrap();
            }
        }
    }
}

async fn simulate(refuse: Vec<[u8; 4]>) -> (u16, Arc<Controller>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let controller = Arc::new(Controller {
        requests: Mutex::default(),
        refuse,
    });
    let c = controller.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(c.clone().serve(stream));
        }
    });
    (port, controller)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
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
async fn commands_are_framed_answered_and_kept_in_state() {
    // Refuse preset switching (register 0x0a000002).
    let (port, controller) = simulate(vec![[0x02, 0x00, 0x00, 0x0a]]).await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "novastar-central-control".into(),
            model: "mx40-pro".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    assert_eq!(
        core.execute(id, "set_brightness", params(json!({"level": 0})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(
            id,
            "set_output_display",
            params(json!({"mode": "blackout"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["brightness"] == 0 && s["output_cards"]["all"]["display"] == "blackout"
    })
    .await;

    match core
        .execute(id, "recall_preset", params(json!({"preset": 1})))
        .await
    {
        Err(CommandError::DeviceError { code, .. }) => assert_eq!(code.as_deref(), Some("1")),
        other => panic!("{other:?}"),
    }

    let requests = controller.requests.lock().unwrap().clone();
    // CCP 1.5.0 3.2.2 and 3.5.2, byte for byte.
    assert_eq!(
        requests[0],
        hex("55 aa 00 00 fe ff 01 ff ff ff 01 00 01 00 00 02 01 00 00 55 5a")
    );
    assert_eq!(
        requests[1],
        hex("55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 10 02 00 ff 01 64 5b")
    );
    // The snapshot carries the last round trip.
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
}
