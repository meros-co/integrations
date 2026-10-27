//! A Pandoras Box master simulated on real TCP, driven through the public API.
//! The simulator reads Pandoras Box Automation messages as the SDK's protocol
//! page frames them (PBAU, version, domain, length, connection ID, protocol,
//! checksum, then the function's code and arguments) and answers the
//! functions that return values, keeping the transport mode set.

#![cfg(feature = "christie-pandorasbox")]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(body: &[u8]) -> Vec<u8> {
    let mut header = vec![1u8, 0, 0, 0, 0];
    header.extend((body.len() as u16).to_be_bytes());
    header.extend([0, 0, 0, 0, 0]);
    let checksum = (header.iter().map(|&b| u32::from(b)).sum::<u32>() % 255) as u8;
    let mut out = b"PBAU".to_vec();
    out.extend(header);
    out.push(checksum);
    out.extend_from_slice(body);
    out
}

fn answer(code: i16, data: &[u8]) -> Vec<u8> {
    let mut body = code.to_be_bytes().to_vec();
    body.extend_from_slice(data);
    frame(&body)
}

fn int(b: &[u8], at: usize) -> i32 {
    i32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

struct Master {
    /// Each sequence's transport mode (1 play, 2 stop, 3 pause).
    transport: Mutex<HashMap<i32, i32>>,
    received: Mutex<Vec<i16>>,
}

impl Master {
    fn reply(&self, code: i16, args: &[u8]) -> Option<Vec<u8>> {
        let mut transport = self.transport.lock().unwrap();
        match code {
            // GetHostRevisionNumber.
            334 => Some(answer(code, &12086i32.to_be_bytes())),
            // GetSiteIds: site 1.
            317 => Some(answer(
                code,
                &[1i32.to_be_bytes(), 1i32.to_be_bytes()].concat(),
            )),
            // GetSelectedDeviceCount.
            81 => Some(answer(code, &2i32.to_be_bytes())),
            // GetIsSiteConnected.
            109 => Some(answer(code, &[1])),
            // GetSequenceTransportMode.
            72 => {
                let mode = *transport.get(&int(args, 0)).unwrap_or(&2);
                Some(answer(code, &mode.to_be_bytes()))
            }
            // GetSequenceTime: 00:00:10:05.
            73 => Some(answer(
                code,
                &[0i32, 0, 10, 5]
                    .iter()
                    .flat_map(|n| n.to_be_bytes())
                    .collect::<Vec<u8>>(),
            )),
            // GetCurrentTimeCueInfo: not on this master.
            295 => Some(answer(-1, &(-3i32).to_be_bytes())),
            // GetCueName.
            277 => {
                let mut d = 5i16.to_be_bytes().to_vec();
                d.extend(b"Intro");
                Some(answer(code, &d))
            }
            // SetSequenceTransportMode: not answered.
            3 => {
                transport.insert(int(args, 0), int(args, 4));
                None
            }
            _ => None,
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
            while buf.len() >= 17 {
                assert_eq!(&buf[..4], b"PBAU", "magic");
                let sum: u32 = buf[4..16].iter().map(|&b| u32::from(b)).sum();
                assert_eq!(buf[16], (sum % 255) as u8, "checksum");
                let len = usize::from(u16::from_be_bytes([buf[9], buf[10]]));
                if buf.len() < 17 + len {
                    break;
                }
                let body: Vec<u8> = buf.drain(..17 + len).skip(17).collect();
                let code = i16::from_be_bytes([body[0], body[1]]);
                self.received.lock().unwrap().push(code);
                if let Some(out) = self.reply(code, &body[2..]) {
                    stream.write_all(&out).await.unwrap();
                }
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Master>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let master = Arc::new(Master {
        transport: Mutex::new(HashMap::from([(1, 1)])),
        received: Mutex::new(Vec::new()),
    });
    let m = master.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(m.clone().serve(stream));
        }
    });
    (port, master)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "christie-pandorasbox".into(),
        model: "pandoras-box".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"sequences": "1", "poll_ms": 200})),
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
    let (port, _master) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);
    wait_for_state(&core, id, |s| {
        s["device"]["revision"] == 12086
            && s["sequences"]["1"]["transport"] == "play"
            && s["sequences"]["1"]["timecode"] == "00:00:10:05"
            && s["sites"]["1"]["connected"] == true
            && s["device"]["selected_devices"] == 2
    })
    .await;
    assert_eq!(
        core.execute(
            id,
            "set_sequence_transport_mode",
            params(json!({"sequence_id": 1, "transport_mode": "pause"}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| s["sequences"]["1"]["transport"] == "pause").await;
    assert_eq!(
        core.execute(
            id,
            "get_cue_name",
            params(json!({"sequence_id": 1, "cue_id": 2}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"cue_name": "Intro"})
        })
    );
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_the_liveness_check_is_sent() {
    let (port, master) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(
            id,
            "get_cue_name",
            params(json!({"sequence_id": 1, "cue_id": 2}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"cue_name": "Intro"})
        })
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut received = master.received.lock().unwrap().clone();
    received.sort();
    // The revision read that shows the master is there, and the command.
    assert_eq!(received, [277, 334], "{received:?}");
    core.close(id).await;
}
