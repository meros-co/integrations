//! A Modbus TCP server simulated on loopback, driven through the public API.
//! The simulator holds 100 coils and 100 holding registers and answers
//! Read Coils (01), Read Holding Registers (03), Write Single Coil (05) and
//! Write Multiple Registers (16) as the MODBUS Application Protocol
//! Specification V1.1b3 lays them out, in MBAP frames, and every other
//! function with exception 01 (illegal function).

#![cfg(feature = "generic-modbus")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Default)]
struct Server {
    coils: Mutex<Vec<bool>>,
    registers: Mutex<Vec<u16>>,
}

impl Server {
    fn answer(&self, pdu: &[u8]) -> Vec<u8> {
        let word = |i: usize| usize::from(u16::from_be_bytes([pdu[i], pdu[i + 1]]));
        match pdu[0] {
            1 => {
                let (start, count) = (word(1), word(3));
                let coils = self.coils.lock().unwrap();
                let mut bytes = vec![0u8; count.div_ceil(8)];
                for i in 0..count {
                    if coils[start + i] {
                        bytes[i / 8] |= 1 << (i % 8);
                    }
                }
                [vec![1, bytes.len() as u8], bytes].concat()
            }
            3 => {
                let (start, count) = (word(1), word(3));
                let regs = self.registers.lock().unwrap();
                let mut out = vec![3, (count * 2) as u8];
                for r in &regs[start..start + count] {
                    out.extend(r.to_be_bytes());
                }
                out
            }
            5 => {
                self.coils.lock().unwrap()[word(1)] = pdu[3] == 0xFF;
                pdu.to_vec()
            }
            16 => {
                let (start, count) = (word(1), word(3));
                let mut regs = self.registers.lock().unwrap();
                for i in 0..count {
                    regs[start + i] = u16::from_be_bytes([pdu[6 + 2 * i], pdu[7 + 2 * i]]);
                }
                pdu[..5].to_vec()
            }
            f => vec![f | 0x80, 1],
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
            while buf.len() >= 7 {
                let len = usize::from(u16::from_be_bytes([buf[4], buf[5]]));
                if buf.len() < 6 + len {
                    break;
                }
                assert_eq!(&buf[2..4], &[0, 0], "protocol identifier 0");
                let frame: Vec<u8> = buf.drain(..6 + len).collect();
                let reply = self.answer(&frame[7..]);
                let mut out = frame[..2].to_vec();
                out.extend([0, 0]);
                out.extend(((reply.len() + 1) as u16).to_be_bytes());
                out.push(frame[6]);
                out.extend(reply);
                stream.write_all(&out).await.unwrap();
            }
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn wait_for(
    core: &Core,
    device: meros_integrations::DeviceId,
    check: impl Fn(&Value) -> bool,
) -> Value {
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
async fn polls_tables_and_writes_through_the_public_api() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = Arc::new(Server {
        coils: Mutex::new(vec![false; 100]),
        registers: Mutex::new((0..100).collect()),
    });
    let s = server.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(s.clone().serve(stream));
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "generic-modbus".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"poll": "co:0:4, level=hr:10:u32", "poll_interval_ms": 100})),
            monitor: true,
        })
        .unwrap();

    // Registers 10 and 11 hold 10 and 11: (10 << 16) | 11.
    let state = wait_for(&core, device, |s| {
        s["values"]["level"]["number"] == json!(655371)
    })
    .await;
    assert_eq!(state["holding_registers"]["10"], json!(10));
    assert_eq!(state["coils"]["3"], json!(false));

    let outcome = core
        .execute(
            device,
            "write_coil",
            params(json!({"address": 3, "value": true})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    assert!(server.coils.lock().unwrap()[3]);
    wait_for(&core, device, |s| s["coils"]["3"] == json!(true)).await;

    let outcome = core
        .execute(
            device,
            "write_value",
            params(json!({"address": 10, "value": 70000, "format": "u32"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(&core, device, |s| {
        s["values"]["level"]["number"] == json!(70000)
    })
    .await;

    let outcome = core
        .execute(
            device,
            "read_holding_registers",
            params(json!({"address": 20, "count": 3})),
        )
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!([20, 21, 22])
        })
    );

    // A function the server lacks fails with its exception.
    let outcome = core
        .execute(device, "read_fifo_queue", params(json!({"address": 0})))
        .await;
    let error = serde_json::to_value(outcome.unwrap_err()).unwrap();
    assert_eq!(error["error"], "device_error");
    assert_eq!(error["code"], "1");
    core.close(device).await;
}
