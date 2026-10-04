//! EasyWorship's remote service simulated on real TCP, driven through the
//! public API: it pairs the controller, sends its status, answers navigation
//! with a new status and takes a status back for the logo.

#![cfg(feature = "softouch-easyworship")]

use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn simulated_easyworship() -> (u16, tokio::sync::mpsc::UnboundedReceiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut pending = String::new();
        let mut slide = 2;
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(end) = pending.find("\r\n") {
                let line = pending[..end].to_string();
                pending.drain(..end + 2);
                let message: Value = serde_json::from_str(&line).unwrap();
                let _ = tx.send(message.clone());
                let status = |slide: i64, logo: bool| {
                    format!(
                        "{}\r\n",
                        json!({"action": "status", "logo": logo, "black": false, "clear": false,
                               "rectype": 1, "pres_no": 1, "slide_no": slide, "requestrev": "8"})
                    )
                };
                let reply = match message["action"].as_str().unwrap_or("") {
                    "connect" => format!(
                        "{}\r\n{}",
                        json!({"action": "paired", "requestrev": "7"}),
                        status(slide, false)
                    ),
                    "nextSlide" => {
                        slide += 1;
                        status(slide, false)
                    }
                    "status" => status(slide, message["logo"] == true),
                    _ => String::new(),
                };
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        }
    });
    (port, rx)
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

#[tokio::test(flavor = "multi_thread")]
async fn easyworship_end_to_end() {
    let (port, mut seen) = simulated_easyworship().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "softouch-easyworship".into(),
            model: "easyworship".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: json!({"device_name": "Booth"}).as_object().unwrap().clone(),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["live"]["slide"] == 2)
    })
    .await;
    let first = seen.recv().await.unwrap();
    assert_eq!(first["action"], "connect");
    assert_eq!(first["uid"], "meros-Booth");

    let outcome = core.execute(id, "next_slide", Default::default()).await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["live"]["slide"] == 3)
    })
    .await;

    let outcome = core
        .execute(
            id,
            "set_logo",
            json!({"enabled": true}).as_object().unwrap().clone(),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["display"]["logo"] == true)
    })
    .await;
    // The status sent back carried the revision EasyWorship last sent.
    let mut status = None;
    while let Ok(m) = seen.try_recv() {
        if m["action"] == "status" {
            status = Some(m);
        }
    }
    let status = status.expect("a status sent back");
    assert_eq!(status["requestrev"], "8");
    assert_eq!(status["rectype"], 1);
    core.close(id).await;
}
