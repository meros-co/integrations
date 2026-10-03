//! A Shure ULX-D simulated on real TCP, driven through the public API.
//!
//! The simulator answers GET n ALL with REP messages run together with no line
//! breaks (as receivers send them), answers SETs with the REP of the new value,
//! sends a SAMPLE once metering is on, and records what it received, so the
//! test can check that metering is turned off again when the device closes.

#![cfg(feature = "shure-wireless")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn simulated_ulxd() -> (u16, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(String::new()));
    let log = received.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let mut pending = String::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            log.lock().unwrap().push_str(&text);
            pending.push_str(&text);
            let mut reply = String::new();
            while let Some(end) = pending.find('>') {
                let message = pending[..=end].trim().to_string();
                pending.drain(..=end);
                match message.as_str() {
                    "< GET 1 ALL >" => reply.push_str(
                        "< REP 1 CHAN_NAME {Pulpit  } >< REP 1 AUDIO_MUTE OFF >\
                         < REP 1 AUDIO_GAIN 024 >< REP 1 FREQUENCY 578350 >< REP 1 BATT_BARS 004 >",
                    ),
                    "< SET 1 METER_RATE 01000 >" => {
                        reply.push_str("< REP 1 METER_RATE 01000 >< SAMPLE 1 ALL AX 078 032 >")
                    }
                    "< SET 1 AUDIO_MUTE ON >" => reply.push_str("< REP 1 AUDIO_MUTE ON >"),
                    _ => {}
                }
            }
            if !reply.is_empty() {
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        }
    });
    (port, received)
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
async fn shure_end_to_end() {
    let (port, received) = simulated_ulxd().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "shure-wireless".into(),
            model: "ulxd4".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["channels"]["1"]["rf"]["rssi_dbm"]["a"] == -50)
    })
    .await;
    let ch = core.snapshot(id).unwrap().state["channels"]["1"].clone();
    assert_eq!(ch["name"], "Pulpit");
    assert_eq!(ch["gain_db"], 6);
    assert_eq!(ch["frequency_khz"], 578350);
    assert_eq!(ch["transmitter"]["battery_bars"], 4);

    let outcome = core
        .execute(id, "mute", params(json!({"channel": 1, "muted": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    assert_eq!(
        core.snapshot(id).unwrap().state["channels"]["1"]["mute"],
        true
    );

    core.close(id).await;
    // The metering subscription is cancelled before the socket closes.
    tokio::time::timeout(Duration::from_secs(2), async {
        while !received
            .lock()
            .unwrap()
            .contains("< SET 1 METER_RATE 00000 >")
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("metering turned off on close");
}
