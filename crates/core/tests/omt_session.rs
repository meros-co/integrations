//! An Open Media Transport source simulated on real TCP, driven through the
//! public API.
//!
//! The simulator behaves as the reference sender does: on accepting the
//! connection it sends its OMTInfo and the current tally, echoes a receiver's
//! tally back as the combined tally once that receiver has subscribed to
//! metadata, and answers an inband VISCA command with a reply carrying the
//! same sequence number.

#![cfg(feature = "omt")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(xml: &str) -> Vec<u8> {
    let mut out = vec![1u8, 1];
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&[0; 2]);
    out.extend_from_slice(&(xml.len() as i32).to_le_bytes());
    out.extend_from_slice(xml.as_bytes());
    out
}

async fn simulated_source() -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let log = received.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut hello =
            frame(r#"<OMTInfo ProductName="Sim" Manufacturer="Meros" Version="1.0" />"#);
        hello.extend(frame(r#"<OMTTally Preview="false" Program=="false" />"#));
        stream.write_all(&hello).await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let mut subscribed = false;
        loop {
            let n = stream.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while buf.len() >= 16 {
                let len = i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
                if buf.len() < 16 + len {
                    break;
                }
                let xml = String::from_utf8(buf[16..16 + len].to_vec()).unwrap();
                buf.drain(..16 + len);
                log.lock().unwrap().push(xml.clone());
                if xml == r#"<OMTSubscribe Metadata="true" />"# {
                    subscribed = true;
                } else if xml.starts_with("<OMTTally") && subscribed {
                    stream.write_all(&frame(&xml)).await.unwrap();
                } else if let Some(rest) =
                    xml.strip_prefix(r#"<OMTPTZ Protocol="VISCA" Sequence=""#)
                {
                    let seq = rest.split('"').next().unwrap();
                    let reply =
                        format!(r#"<OMTPTZ Protocol="VISCA" Sequence="{seq}" Reply="9041FF" />"#);
                    stream.write_all(&frame(&reply)).await.unwrap();
                }
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
async fn omt_source_end_to_end() {
    let (port, received) = simulated_source().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "omt".into(),
            model: "sender".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["info"]["product_name"] == "Sim")
    })
    .await;

    let outcome = core
        .execute(id, "set_tally", params(json!({"program": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["tally"]["program"] == true)
    })
    .await;

    let outcome = core
        .execute(id, "ptz_visca", params(json!({"command": "8101040700FF"})))
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!("9041FF")
        })
    );
    let log = received.lock().unwrap().clone();
    assert_eq!(log[0], r#"<OMTSubscribe Metadata="true" />"#);
    assert!(log.contains(&r#"<OMTTally Preview="false" Program=="true" />"#.to_string()));
}
