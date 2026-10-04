//! A Magewell Pro Convert decoder simulated on real TCP, driven through the
//! public API.
//!
//! The simulator answers each HTTP request on its own connection, as the
//! device's web server does with Connection close: login sets a session
//! cookie, and every other method needs it.

#![cfg(feature = "magewell-proconvert")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn simulated_decoder() -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let request = String::from_utf8_lossy(&buf).to_string();
            let line = request.lines().next().unwrap_or("").to_string();
            log.lock().unwrap().push(request.clone());
            let authed = request.contains("Cookie: sid=s1");
            let (cookie, body) = if line.contains("method=login") {
                ("Set-Cookie: sid=s1; path=/\r\n", json!({"status": 0}))
            } else if !authed {
                ("", json!({"status": 37}))
            } else if line.contains("method=get-summary-info") {
                (
                    "",
                    json!({"status": 0, "device": {"model": "NDI to HDMI", "fw-version": "1.3.100"},
                           "ndi": {"name": "STUDIO (Camera 1)", "tally-program": true, "tally-preview": false}}),
                )
            } else {
                ("", json!({"status": 0}))
            };
            let body = body.to_string();
            let reply = format!(
                "HTTP/1.1 200 OK\r\n{cookie}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes()).await;
        }
    });
    (port, requests)
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
async fn proconvert_decoder_end_to_end() {
    let (port, requests) = simulated_decoder().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "magewell-proconvert".into(),
            model: "ndi-to-hdmi".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["ndi"]["name"] == "STUDIO (Camera 1)" && patch["tally"]["program"] == true)
    })
    .await;

    let outcome = core
        .execute(
            id,
            "select_ndi_source",
            params(json!({"name": "STUDIO (Camera 2)"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    let log = requests.lock().unwrap().clone();
    assert!(log[0]
        .starts_with("GET /mwapi?method=login&id=Admin&pass=e3afed0047b08059d0fada10f400c1e5 "));
    assert!(log.iter().any(|r| r.starts_with(
        "GET /mwapi?method=set-channel&ndi-name=true&name=STUDIO%20%28Camera%202%29 "
    ) && r.contains("Cookie: sid=s1")));
}
