//! vMix simulated on real TCP, driven through the public API.
//!
//! The simulator sends the unrequested VERSION line, answers SUBSCRIBE, XML
//! (with the byte count the TCP API specifies) and FUNCTION, and pushes a
//! TALLY event after a cut, before the FUNCTION reply, as the API allows.

#![cfg(feature = "vmix")]

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const XML: &str = r#"<vmix><version>27.0.0.49</version><edition>4K</edition><inputs><input key="a" number="1" type="Capture" title="Camera 1" state="Running"/><input key="b" number="2" type="Capture" title="Camera 2" state="Running"/></inputs><preview>2</preview><active>1</active><recording>False</recording></vmix>"#;

async fn simulated_vmix() -> u16 {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let (read, mut write) = tcp.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"VERSION OK 27.0.0.49\r\n").await.unwrap();
        while let Ok(Some(line)) = lines.next_line().await {
            let reply = if let Some(what) = line.strip_prefix("SUBSCRIBE ") {
                format!("SUBSCRIBE OK {what}\r\n")
            } else if line == "XML" {
                format!("XML {}\r\n{XML}\r\n", XML.len() + 2)
            } else if line == "FUNCTION Cut" {
                // Input 2 goes to program and 1 to preview; the tally event
                // arrives before the reply.
                "TALLY OK 21\r\nFUNCTION OK Completed\r\n".to_string()
            } else if line.starts_with("FUNCTION ") {
                "FUNCTION ER Unknown function\r\n".to_string()
            } else {
                continue;
            };
            write.write_all(reply.as_bytes()).await.unwrap();
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
async fn vmix_end_to_end() {
    let port = simulated_vmix().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "vmix".into(),
            model: "vmix".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id && patch["inputs"]["2"]["title"] == "Camera 2")
    })
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.connection, Connection::Connected);
    assert_eq!(snapshot.state["program"], 1);
    assert_eq!(snapshot.state["device"]["edition"], "4K");

    assert_eq!(
        core.execute(id, "cut", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    let tally = &core.snapshot(id).unwrap().state["tally"];
    assert_eq!(tally["1"]["preview"], true);
    assert_eq!(tally["2"]["program"], true);

    let outcome = core.execute(id, "fade_to_black", params(json!({}))).await;
    assert!(matches!(outcome, Err(CommandError::DeviceError { .. })));

    core.close(id).await;
}
