//! A VDCP server simulated on loopback TCP, driven through the public API.
//!
//! The server checks each message's checksum, grants the port the controller
//! opens, keeps a list of IDs, cues and plays one, and answers the port
//! status, position, active ID, device type, system status and ID list
//! (ten at a time, with NEXT) requests.

#![cfg(feature = "vdcp")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Core, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn check(bytes: &[u8]) -> u8 {
    bytes
        .iter()
        .fold(0u8, |a, b| a.wrapping_add(*b))
        .wrapping_neg()
}

fn frame(cmd1: u8, cmd2: u8, data: &[u8]) -> Vec<u8> {
    let mut body = vec![cmd1, cmd2];
    body.extend_from_slice(data);
    let mut out = vec![0x02, body.len() as u8];
    out.extend_from_slice(&body);
    out.push(check(&body));
    out
}

#[derive(Default)]
struct Server {
    ids: Vec<String>,
    cued: Option<String>,
    playing: Option<String>,
    added: Vec<String>,
    received: Vec<(u8, u8)>,
}

impl Server {
    fn pad(id: &str) -> Vec<u8> {
        let mut v = id.as_bytes().to_vec();
        v.resize(8, b' ');
        v
    }

    fn answer(&mut self, cmd1: u8, cmd2: u8, data: &[u8]) -> Vec<u8> {
        self.received.push((cmd1, cmd2));
        match (cmd1 >> 4, cmd2) {
            (3, 0x01) => frame(0x30, 0x81, &[1]),
            (3, 0x05) => {
                let s1 = if self.playing.is_some() {
                    0x04
                } else if self.cued.is_some() {
                    0x80
                } else {
                    0x01
                };
                let s2 = if self.added.is_empty() { 0 } else { 0x02 };
                frame(0x30, 0x85, &[data[0], s1, 0x01, s2, 0, 0, 0, 0x10])
            }
            (3, 0x06) => frame(0x30, 0x86, &[data[0], 0x10, 0x20, 0x00, 0x01]),
            (3, 0x07) => match &self.playing {
                Some(id) => {
                    let mut v = vec![1];
                    v.extend(Self::pad(id));
                    frame(0x30, 0x87, &v)
                }
                None => frame(0x30, 0x87, &[0]),
            },
            (3, 0x08) => frame(0x30, 0x88, b"SIM:DDR"),
            (3, 0x10) => frame(0x30, 0x90, &[0x02, 0, self.ids.len() as u8]),
            (3, 0x11) | (3, 0x02) => {
                // Two at a time, to exercise NEXT.
                let take: Vec<String> = self
                    .ids
                    .iter()
                    .skip(if cmd2 == 0x02 { 2 } else { 0 })
                    .take(2)
                    .cloned()
                    .collect();
                let remaining = self
                    .ids
                    .len()
                    .saturating_sub(if cmd2 == 0x02 { 4 } else { 2 })
                    as u16;
                let mut v = remaining.to_be_bytes().to_vec();
                for id in &take {
                    v.extend(Self::pad(id));
                }
                frame(0x30, cmd2 | 0x80, &v)
            }
            (3, 0x18) => {
                let mut v = vec![0, 0];
                for id in self.added.drain(..) {
                    v.extend(Self::pad(&id));
                }
                frame(0x30, 0x98, &v)
            }
            (2, 0x24) => {
                self.cued = Some(String::from_utf8_lossy(&data[..8]).trim_end().to_string());
                vec![0x04]
            }
            (2, 0x22) => vec![0x04],
            (2, 0x23) => {
                let id = String::from_utf8_lossy(&data[..8]).trim_end().to_string();
                self.ids.push(id.clone());
                self.added.push(id);
                vec![0x04]
            }
            (1, 0x01) => {
                self.playing = self.cued.take();
                vec![0x04]
            }
            (1, 0x00) => {
                self.playing = None;
                vec![0x04]
            }
            _ => vec![0x05, 0x01],
        }
    }
}

async fn simulate() -> (u16, Arc<Mutex<Server>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = Arc::new(Mutex::new(Server {
        ids: vec!["SPOT1".into(), "SPOT2".into(), "SPOT3".into()],
        ..Default::default()
    }));
    let s = server.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let s = s.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 256];
                loop {
                    let n = stream.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    while buf.len() >= 2 {
                        if buf[0] == 0x04 {
                            buf.remove(0);
                            continue;
                        }
                        assert_eq!(buf[0], 0x02, "STX");
                        let len = 2 + buf[1] as usize + 1;
                        if buf.len() < len {
                            break;
                        }
                        let f: Vec<u8> = buf.drain(..len).collect();
                        assert_eq!(check(&f[2..len - 1]), f[len - 1], "checksum");
                        let answer = s.lock().unwrap().answer(f[2], f[3], &f[4..len - 1]);
                        stream.write_all(&answer).await.unwrap();
                    }
                }
            });
        }
    });
    (port, server)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_is_read_cued_and_played() {
    let (port, _server) = simulate().await;
    let core: Core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "vdcp".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"link": "tcp", "port": 1})),
            monitor: true,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["port"]["granted"] == true
                && s["device_type"] == "SIM:DDR"
                && s["ids"] == json!({"SPOT1": true, "SPOT2": true, "SPOT3": true})
                && s["port_status"]["state"] == "idle"
            {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s");
    assert_eq!(
        core.execute(id, "play_cue", params(json!({"id": "SPOT2"})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "play", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["port_status"]["state"] == "play_record"
                && s["active_id"] == "SPOT2"
                && s["position"]["timecode"] == "01:00:20:10"
            {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("playing, with its position");
    // A recording adds an ID, which the added list brings into state.
    assert_eq!(
        core.execute(
            id,
            "record_init",
            params(json!({"id": "NEW1", "duration": "00:00:30:00"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().state["ids"]["NEW1"] != true {
            core.next_events(64).await;
        }
    })
    .await
    .expect("the added ID in state");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_the_server_is_not_polled() {
    let (port, server) = simulate().await;
    let core: Core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "vdcp".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"link": "tcp", "port": 1})),
            monitor: false,
        })
        .unwrap();
    assert_eq!(
        core.execute(id, "stop", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(600)).await;
    // The port is opened and selected first; the command may go ahead of the
    // device type request.
    let mut received = server.lock().unwrap().received.clone();
    received.sort();
    assert_eq!(
        received,
        [(0x10, 0x00), (0x20, 0x22), (0x30, 0x01), (0x30, 0x08)]
    );
    core.close(id).await;
}
