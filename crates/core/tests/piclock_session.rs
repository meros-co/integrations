//! A PiClock simulated on real TCP, driven through the public API. The
//! simulated clock connects to the core as the Tally Protocol describes,
//! answers its CRYPT challenge with the SHA-512 digest of the challenge and
//! the shared secret, answers PING with PONG and every command with ACK, and
//! sends its GPI state once authenticated.

#![cfg(feature = "piclock")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{OpenRequest, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha512};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn clock(port: u16, secret: &'static str, received: Arc<Mutex<Vec<String>>>) {
    let stream = loop {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => break s,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    };
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).split(b'\r');
    while let Ok(Some(raw)) = lines.next_segment().await {
        let line = String::from_utf8(raw).unwrap();
        received.lock().unwrap().push(line.clone());
        let answer = if let Some(challenge) = line.strip_prefix("CRYPT:") {
            let digest: String = Sha512::digest(format!("{challenge}{secret}").as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            format!("AUTH:{digest}:b827eb0a0b0c\rGPI:3\r")
        } else if line == "PING" {
            "PONG\r".into()
        } else {
            "ACK\r".into()
        };
        if write.write_all(answer.as_bytes()).await.is_err() {
            return;
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clock_authenticates_and_takes_commands() {
    let port = free_port();
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "piclock".into(),
            model: "piclock".into(),
            host: "127.0.0.1".into(),
            port: None,
            settings: params(json!({"password": "studio", "listen_port": port, "ping_ms": 500})),
            monitor: true,
        })
        .unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(clock(port, "studio", received.clone()));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let s = core.snapshot(id).unwrap().state;
            if s["mac_address"] == "B827EB0A0B0C" && s["gpi"]["1"] == true {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("authenticated within 10 s");
    assert_eq!(
        core.execute(
            id,
            "set_tally",
            params(json!({"row": 0, "column": 0, "foreground": "FFFFFF",
                          "background": "FF0000", "text": "ON AIR"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(700)).await;
    let received = received.lock().unwrap().clone();
    assert!(received.contains(&"0SETTALLY:0:0:#FFFFFF:#FF0000:ON AIR".to_string()));
    assert!(received.contains(&"PING".to_string()), "{received:?}");
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clock_with_another_secret_is_refused() {
    let port = free_port();
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "piclock".into(),
            model: "piclock".into(),
            host: "127.0.0.1".into(),
            port: None,
            settings: params(json!({"password": "studio", "listen_port": port})),
            monitor: true,
        })
        .unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(clock(port, "other", received));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(core
        .execute(id, "clear_images", params(json!({})))
        .await
        .is_err());
    assert!(core
        .snapshot(id)
        .unwrap()
        .state
        .get("mac_address")
        .is_none());
    core.close(id).await;
}
