//! A pigpio daemon simulated on loopback, driven through the public API. The
//! simulator answers the socket interface's four-word requests as the pigpio
//! documentation gives them: NOIB on a connection makes it a notification
//! stream with handle 0, NB starts reports, MODEG answers input, BR1 the
//! levels, and WRITE sets a level and sends a 12-byte report on the stream.

#![cfg(feature = "pigpio")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Default)]
struct Daemon {
    levels: Mutex<u32>,
    notify: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
}

impl Daemon {
    async fn serve(self: Arc<Self>, stream: tokio::net::TcpStream) {
        let (mut rd, mut wr) = stream.into_split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(b) = rx.recv().await {
                if wr.write_all(&b).await.is_err() {
                    return;
                }
            }
        });
        let mut buf = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            let n = rd.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while buf.len() >= 16 {
                let w: Vec<u32> = buf[..16]
                    .chunks(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let ext = w[3] as usize;
                if buf.len() < 16 + ext {
                    break;
                }
                buf.drain(..16 + ext);
                let res: u32 = match w[0] {
                    99 => {
                        *self.notify.lock().unwrap() = Some(tx.clone());
                        0
                    }
                    1 => 0,
                    10 => *self.levels.lock().unwrap(),
                    4 => {
                        let mut levels = self.levels.lock().unwrap();
                        if w[2] == 1 {
                            *levels |= 1 << w[1];
                        } else {
                            *levels &= !(1 << w[1]);
                        }
                        if let Some(n) = self.notify.lock().unwrap().as_ref() {
                            let mut r = 1u16.to_le_bytes().to_vec();
                            r.extend(0u16.to_le_bytes());
                            r.extend(1234u32.to_le_bytes());
                            r.extend(levels.to_le_bytes());
                            let _ = n.send(r);
                        }
                        0
                    }
                    17 => 0xa02082,
                    26 => 79,
                    103 => 8,
                    _ => 0,
                };
                let mut out = Vec::new();
                for x in [w[0], w[1], w[2], res] {
                    out.extend(x.to_le_bytes());
                }
                let _ = tx.send(out);
            }
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_levels_from_notifications_and_writes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let daemon = Arc::new(Daemon::default());
    let d = daemon.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(d.clone().serve(stream));
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "pigpio".into(),
            model: "pigpiod".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"gpios": "4-6"})),
            monitor: true,
        })
        .unwrap();

    let wait = |check: fn(&Value) -> bool| {
        let core = &core;
        async move {
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
    };
    let state = wait(|s| s["gpio"]["5"]["mode"] == json!("input")).await;
    assert_eq!(state["hardware_revision"], "a02082");
    assert_eq!(state["gpio"]["5"]["level"], false);

    // Wait for the notification stream to be set up before writing.
    tokio::time::timeout(Duration::from_secs(10), async {
        while daemon.notify.lock().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let outcome = core
        .execute(device, "write", params(json!({"gpio": 6, "level": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait(|s| s["gpio"]["6"]["level"] == json!(true)).await;
    core.close(device).await;
}
