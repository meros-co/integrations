//! A Biamp Tesira simulated on real TCP, driven through the public API.
//!
//! The simulator opens with Telnet option negotiation and sends the welcome
//! banner only once every option has been refused, as Biamp documents; then
//! it answers TTP lines and publishes a subscription's value.

#![cfg(feature = "biamp-tesira")]

use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn simulated_tesira() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // DO terminal type, WILL echo.
        stream
            .write_all(&[255, 253, 0x18, 255, 251, 0x01])
            .await
            .unwrap();
        let mut refused = Vec::new();
        let mut buf = [0u8; 1024];
        while refused.len() < 6 {
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0, "closed during negotiation");
            refused.extend_from_slice(&buf[..n]);
        }
        assert_eq!(refused, [255, 252, 0x18, 255, 254, 0x01]);
        stream
            .write_all(b"\r\nWelcome to the Tesira Text Protocol Server...\r\n")
            .await
            .unwrap();
        let mut pending = String::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(end) = pending.find('\n') {
                let line: String = pending.drain(..=end).collect();
                let reply = match line.trim() {
                    "DEVICE get serialNumber" => "+OK \"value\":\"01842224\"\r\n".to_string(),
                    "DEVICE get version" => "+OK \"value\":\"4.2.0.21341\"\r\n".to_string(),
                    "DEVICE get hostname" => "+OK \"value\":\"TesiraForte1\"\r\n".to_string(),
                    "Level1 subscribe level 1 meros1" => {
                        "! \"publishToken\":\"meros1\" \"value\":-10.000000\r\n+OK\r\n".to_string()
                    }
                    "Level1 set level 1 -20" => {
                        "+OK\r\n! \"publishToken\":\"meros1\" \"value\":-20.000000\r\n".to_string()
                    }
                    "Mute1 get mutes" => "+OK \"value\":[false true]\r\n".to_string(),
                    _ => "+OK\r\n".to_string(),
                };
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        }
    });
    port
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event within 10 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn tesira_end_to_end() {
    let port = simulated_tesira().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "biamp-tesira".into(),
            model: "tesiraforte".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"subscriptions": "Level1 level 1"})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["blocks"]["Level1"]["level"]["1"] == -10.0)
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["serial_number"], "01842224");
    assert_eq!(state["device"]["hostname"], "TesiraForte1");

    let outcome = core
        .execute(
            id,
            "set_level",
            params(json!({"tag": "Level1", "channel": 1, "level_db": -20.0})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["blocks"]["Level1"]["level"]["1"] == -20.0)
    })
    .await;

    let outcome = core
        .execute(
            id,
            "get",
            params(json!({"tag": "Mute1", "attribute": "mutes"})),
        )
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!([false, true])
        })
    );
    assert_eq!(
        core.snapshot(id).unwrap().state["blocks"]["Mute1"]["mutes"],
        json!([false, true])
    );
    core.close(id).await;
}
