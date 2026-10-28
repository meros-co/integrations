//! A JNIOR simulated on loopback, driven through the public API. The
//! simulator speaks the JANOS Management Protocol as the JNIOR Users Manual
//! lays it out: [length, object] wrapped JSON on TCP, a 401 challenge with a
//! nonce answered by the MD5 digest, Authenticated then Monitor, Control
//! Close answered by a Monitor showing the relay closed, and Registry Read
//! answered with the request's Meta.

#![cfg(feature = "integ-jnior")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn wrap(v: &Value) -> Vec<u8> {
    let t = v.to_string();
    format!("[{},{}]", t.len(), t).into_bytes()
}

#[derive(Default)]
struct Unit {
    relays: Mutex<[u8; 8]>,
}

impl Unit {
    fn monitor(&self) -> Value {
        let relays = self.relays.lock().unwrap();
        json!({"Message": "Monitor", "Model": "410", "Version": "v2.4", "Serial Number": 618000001,
               "Inputs": (0..8).map(|i| json!({"State": i % 2, "Count": i})).collect::<Vec<_>>(),
               "Outputs": relays.iter().map(|s| json!({"State": s})).collect::<Vec<_>>(),
               "Timestamp": 1_700_000_000_000u64})
    }

    fn answer(&self, msg: &Value) -> Vec<Value> {
        match msg.get("Message").and_then(Value::as_str) {
            Some("") => vec![
                json!({"Message": "Error", "Text": "401 Unauthorized", "Nonce": "bc581a9683d3e1857218db135e4b"}),
            ],
            Some("Control") if msg["Command"] == "Close" => {
                let ch = msg["Channel"].as_u64().unwrap() as usize;
                self.relays.lock().unwrap()[ch - 1] = 1;
                vec![self.monitor()]
            }
            Some("Registry Read") => {
                let keys: serde_json::Map<String, Value> = msg["Keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| {
                        let k = k.as_str().unwrap().to_string();
                        let v = if k == "IO/Inputs/din1/Desc" {
                            "Door"
                        } else {
                            ""
                        };
                        (k, json!(v))
                    })
                    .collect();
                vec![json!({"Message": "Registry Response", "Meta": msg["Meta"], "Keys": keys})]
            }
            Some("Enumerate Devices") => {
                vec![
                    json!({"Message": "Enumerate Devices Response", "Meta": msg["Meta"], "Devices": []}),
                ]
            }
            Some("Clock Read") => {
                vec![
                    json!({"Message": "Clock Response", "Meta": msg["Meta"], "Time": 1_700_000_000_000u64, "Date": "Tue, 14 Nov 2023 22:13:20 GMT"}),
                ]
            }
            None if msg.get("Auth-Digest").is_some() => {
                // The manual's example: user jnior, password jnior.
                assert_eq!(msg["Auth-Digest"], "jnior:6b7b418f223e7e0dc600c41c7b6644b3");
                vec![
                    json!({"Message": "Authenticated", "Administrator": true, "Control": true}),
                    self.monitor(),
                ]
            }
            _ => vec![],
        }
    }

    async fn serve(self: Arc<Self>, mut stream: tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = stream.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            while let Some(comma) = buf.iter().position(|&b| b == b',') {
                let len: usize = std::str::from_utf8(&buf[1..comma])
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                if buf.len() < comma + 1 + len + 1 {
                    break;
                }
                let msg: Value = serde_json::from_slice(&buf[comma + 1..comma + 1 + len]).unwrap();
                assert_eq!(buf[comma + 1 + len], b']');
                buf.drain(..comma + 2 + len);
                for reply in self.answer(&msg) {
                    stream.write_all(&wrap(&reply)).await.unwrap();
                }
            }
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_in_keeps_io_and_closes_a_relay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let unit = Arc::new(Unit::default());
    let u = unit.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(u.clone().serve(stream));
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "integ-jnior".into(),
            model: "jnior-410".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();

    let wait = |check: fn(&Value) -> bool| {
        let core = &core;
        async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let state = core.snapshot(device).unwrap().state;
                    if check(&state) {
                        return state;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("state")
        }
    };
    let state = wait(|s| s["inputs"]["1"]["description"] == json!("Door")).await;
    assert_eq!(state["device"]["model"], "410");
    assert_eq!(state["inputs"]["2"]["state"], true);
    assert_eq!(state["login"]["administrator"], true);

    let outcome = core
        .execute(device, "close_relay", params(json!({"channel": 3})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Unverified));
    wait(|s| s["outputs"]["3"]["state"] == json!(true)).await;

    let outcome = core
        .execute(
            device,
            "registry_read",
            params(json!({"keys": "IO/Inputs/din1/Desc"})),
        )
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!({"IO/Inputs/din1/Desc": "Door"})
        })
    );
    core.close(device).await;
}
