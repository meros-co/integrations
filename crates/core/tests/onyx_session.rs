//! ONYX Manager's Telnet server simulated on real TCP, driven through the
//! public API: a banner on connecting, "200 Ok" replies with data lines ended
//! by ".", and an error packet for commands it does not know.

#![cfg(feature = "obsidian-onyx")]

use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn simulated_manager() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"200-*****Welcome to Onyx Manager*****\r\n200-Type HELP for a list of available commands\r\n200\r\n")
            .await
            .unwrap();
        let mut active = false;
        let mut buf = [0u8; 1024];
        let mut pending = String::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(end) = pending.find("\r\n") {
                let line = pending[..end].to_string();
                pending.drain(..end + 2);
                let reply = match line.as_str() {
                    "QLList" => "200 Ok\r\n00002 - House Lights\r\n00014 - Walk In\r\n.\r\n".to_string(),
                    "QLActive" if active => "200 Ok\r\n00014 - Walk In\r\n.\r\n".to_string(),
                    "QLActive" => "200 Ok\r\nNo Active Qlist in List\r\n.\r\n".to_string(),
                    "IsMxRun" => "200 Ok\r\nYes\r\n.\r\n".to_string(),
                    "IsSchRun" => "200 Ok\r\nNo\r\n.\r\n".to_string(),
                    "GQL 14" => {
                        active = true;
                        "200 Ok\r\n.\r\n".to_string()
                    }
                    other => format!(
                        "400-I never heard that command before... are you sure?\r\n400-Type HELP for a list of commands\r\n400 {other}\r\n"
                    ),
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
async fn onyx_manager_end_to_end() {
    let port = simulated_manager().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "obsidian-onyx".into(),
            model: "onyx-manager".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["cuelists"]["14"]["active"] == false)
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["cuelists"]["2"]["name"], "House Lights");

    let outcome = core
        .execute(id, "go_cuelist", params(json!({"cuelist": 14})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    // The active list is asked again soon after a cuelist command.
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["cuelists"]["14"]["active"] == true)
    })
    .await;

    let outcome = core
        .execute(id, "is_onyx_running", Default::default())
        .await;
    assert_eq!(outcome, Ok(Outcome::Value { value: json!(true) }));

    let outcome = core.execute(id, "status", Default::default()).await;
    assert!(
        matches!(&outcome, Err(e) if e.to_string().contains("never heard")),
        "{outcome:?}"
    );
    core.close(id).await;
}
