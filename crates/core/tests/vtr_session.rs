//! A Sony 9-pin deck behind a serial-to-Ethernet converter, and an AMP server's
//! TCP socket, simulated on loopback and driven through the public API.
//!
//! The deck checks each message's checksum, answers ACK, NAK, its device type,
//! its status bytes and its time, and keeps play and stop and an in point. The
//! AMP server answers the channel request with 1001 and the same messages in
//! hexadecimal, and keeps a clip list and the loaded clip.

#![cfg(any(feature = "sony-9pin", feature = "grassvalley-amp"))]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Core, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[cfg(feature = "sony-9pin")]
fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

#[derive(Default)]
struct Deck {
    playing: bool,
    in_point: [u8; 4],
    loaded: String,
    received: Vec<Vec<u8>>,
}

impl Deck {
    /// The answer to one message (without its checksum).
    fn answer(&mut self, m: &[u8]) -> Vec<u8> {
        self.received.push(m.to_vec());
        let ack = vec![0x10, 0x01];
        match (m[0], m[1]) {
            (0x00, 0x11) => vec![0x12, 0x11, 0x20, 0x25],
            (0x20, 0x01) => {
                self.playing = true;
                ack
            }
            (0x20, 0x00) => {
                self.playing = false;
                ack
            }
            (0x61, 0x20) => {
                let mut v = vec![0x70 | (m[2] & 0x0F), 0x20];
                let mut bytes = [0u8; 15];
                bytes[1] = if self.playing { 0x01 } else { 0x20 };
                v.extend_from_slice(&bytes[..(m[2] & 0x0F) as usize]);
                v
            }
            (0x61, 0x0C) => vec![0x74, 0x04, 0x10, 0x20, 0x30, 0x01],
            (0x44, 0x14) => {
                self.in_point.copy_from_slice(&m[2..6]);
                ack
            }
            (0x60, 0x10) => {
                let mut v = vec![0x74, 0x10];
                v.extend_from_slice(&self.in_point);
                v
            }
            (0x60, _) | (0x61, _) => vec![0x74, m[1], 0, 0, 0, 0],
            (0xA0, 0x21) => {
                let mut v = vec![0x88, 0x21];
                v.extend_from_slice(b"K2-SIM  ");
                v
            }
            (0xA0, 0x16) => {
                let name = self.loaded.as_bytes();
                let mut v = vec![0x82, 0x16];
                v.extend_from_slice(&((name.len() + 2) as u16).to_be_bytes());
                v.extend_from_slice(&(name.len() as u16).to_be_bytes());
                v.extend_from_slice(name);
                v
            }
            (0x4A, 0x14) => {
                let len = u16::from_be_bytes([m[4], m[5]]) as usize;
                self.loaded = String::from_utf8_lossy(&m[6..6 + len]).into_owned();
                ack
            }
            (0xA0, 0x0F) => {
                let mut v = vec![0x82, 0x0F, 0x00, 0x0C, 0x00, 0x0A];
                v.extend_from_slice(b"V:/default");
                v
            }
            (0xA2, 0x14) => {
                let mut v = vec![0x8A, 0x14, 0x00, 0x0A];
                v.extend_from_slice(&[0, 3]);
                v.extend_from_slice(b"one");
                v.extend_from_slice(&[0, 3]);
                v.extend_from_slice(b"two");
                v
            }
            (0xA1, 0x15) => vec![0x80, 0x14],
            _ => vec![0x11, 0x12, 0x01],
        }
    }
}

async fn serve<F>(mut handle: F) -> u16
where
    F: FnMut(tokio::net::TcpStream) -> tokio::task::JoinHandle<()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            handle(stream);
        }
    });
    port
}

#[cfg(feature = "sony-9pin")]
/// A deck behind a converter: binary messages with a checksum.
async fn binary_deck() -> (u16, Arc<Mutex<Deck>>) {
    let deck = Arc::new(Mutex::new(Deck::default()));
    let d = deck.clone();
    let port = serve(move |mut stream| {
        let d = d.clone();
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
                    let len = 2 + (buf[0] & 0x0F) as usize + 1;
                    if buf.len() < len {
                        break;
                    }
                    let frame: Vec<u8> = buf.drain(..len).collect();
                    assert_eq!(sum(&frame[..len - 1]), frame[len - 1], "checksum");
                    let mut answer = d.lock().unwrap().answer(&frame[..len - 1]);
                    answer.push(sum(&answer));
                    stream.write_all(&answer).await.unwrap();
                }
            }
        })
    })
    .await;
    (port, deck)
}

#[cfg(feature = "grassvalley-amp")]
/// An AMP server's socket: CRAT, then CMDS messages in hexadecimal.
async fn amp_server() -> (u16, Arc<Mutex<Deck>>) {
    let deck = Arc::new(Mutex::new(Deck::default()));
    let d = deck.clone();
    let port = serve(move |mut stream| {
        let d = d.clone();
        tokio::spawn(async move {
            let mut text = String::new();
            let mut chunk = [0u8; 512];
            loop {
                let n = stream.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                text.push_str(&String::from_utf8_lossy(&chunk[..n]));
                while let Some(end) = text.find('\n') {
                    let line: String = text.drain(..=end).collect();
                    let line = line.trim_end();
                    let reply = if line.starts_with("CRAT") {
                        assert_eq!(line, "CRAT0007204Vtr1");
                        "1001".to_string()
                    } else if let Some(rest) = line.strip_prefix("CMDS") {
                        let len: usize = rest[..4].parse().unwrap();
                        let hex = &rest[4..];
                        assert_eq!(hex.len(), len, "length");
                        let bytes: Vec<u8> = (0..hex.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                            .collect();
                        let answer = d.lock().unwrap().answer(&bytes);
                        answer.iter().map(|b| format!("{b:02x}")).collect()
                    } else {
                        continue;
                    };
                    stream.write_all(reply.as_bytes()).await.unwrap();
                }
            }
        })
    })
    .await;
    (port, deck)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[cfg(feature = "sony-9pin")]
#[tokio::test(flavor = "multi_thread")]
async fn a_9pin_deck_is_read_and_driven() {
    let (port, deck) = binary_deck().await;
    let core: Core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "sony-9pin".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"link": "tcp"})),
            monitor: true,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["device"]["model"] == "BVW-75"
                && s["transport"] == "stop"
                && s["timecode"] == "01:30:20:10"
            {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s");
    assert_eq!(
        core.execute(id, "play", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().state["transport"] != "play" {
            core.next_events(64).await;
        }
    })
    .await
    .expect("the status shows play");
    assert_eq!(
        core.execute(
            id,
            "set_point",
            params(json!({"point": "in", "timecode": "00:01:02:03"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().state["points"]["in"] != "00:01:02:03" {
            core.next_events(64).await;
        }
    })
    .await
    .expect("the in point read back");
    assert!(deck
        .lock()
        .unwrap()
        .received
        .iter()
        .any(|m| m[..2] == [0x44, 0x14]));
    core.close(id).await;
}

#[cfg(feature = "sony-9pin")]
#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_the_deck_is_not_polled() {
    let (port, deck) = binary_deck().await;
    let core: Core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "sony-9pin".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"link": "tcp"})),
            monitor: false,
        })
        .unwrap();
    assert_eq!(
        core.execute(id, "stop", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(600)).await;
    let received = deck.lock().unwrap().received.clone();
    assert!(
        received
            .iter()
            .all(|m| m[..2] == [0x00, 0x11] || m[..2] == [0x20, 0x00]),
        "{received:?}"
    );
    core.close(id).await;
}

#[cfg(feature = "grassvalley-amp")]
#[tokio::test(flavor = "multi_thread")]
async fn an_amp_server_loads_and_lists_clips() {
    let (port, _server) = amp_server().await;
    let core: Core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "grassvalley-amp".into(),
            model: "k2".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"channel": "Vtr1"})),
            monitor: true,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["device_id"] == "K2-SIM" && s["transport"] == "stop" {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s");
    assert_eq!(
        core.execute(id, "load_clip", params(json!({"clip": "clip01"})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().state["clip"]["loaded"] != "clip01" {
            core.next_events(64).await;
        }
    })
    .await
    .expect("the loaded clip read back");
    assert_eq!(
        core.execute(id, "list_clips", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!(["one", "two"])
        })
    );
    assert_eq!(
        core.snapshot(id).unwrap().state["clips"],
        json!({"1": "one", "2": "two"})
    );
    core.close(id).await;
}
