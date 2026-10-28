//! An AdderLink Infinity Manager simulated as a loopback HTTP server, driven
//! through the public API. The simulator answers the AIM API's GET
//! /api/?v=14&method=... with the XML answers appendix K of the A.I.M. user
//! guide shows: a login with a token, get_devices listing one receiver,
//! empty lists for the rest, and connect_channel, which it records and only
//! accepts with the token.

#![cfg(feature = "adder-aim")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const TOKEN: &str = "5cf494a71c29e9465a57a81e0a2d602c";

fn answer(query: &str, connected: &Mutex<Option<String>>) -> String {
    let get = |k: &str| {
        query
            .split('&')
            .find_map(|p| p.strip_prefix(&format!("{k}=")))
            .unwrap_or("")
            .to_string()
    };
    let ok = |inner: &str| {
        format!("<api_response><version>14</version><timestamp>2026-10-28 10:00:00</timestamp><success>1</success>{inner}</api_response>")
    };
    match get("method").as_str() {
        "login" => ok(&format!("<token>{TOKEN}</token>")),
        _ if get("token") != TOKEN => "<api_response><success>0</success><errors><error><code>1</code><msg>Not logged in</msg></error></errors></api_response>".into(),
        "get_devices" if get("device_type") == "rx" => {
            let channel = connected.lock().unwrap().clone().unwrap_or_default();
            ok(&format!("<page>1</page><results_per_page>1000</results_per_page><total_devices>1</total_devices><count_devices>1</count_devices><devices><device item=\"1\"><d_id>170</d_id><d_name>RX 123</d_name><d_status>1</d_status><con_c_id>{channel}</con_c_id><con_end_time/><con_control>3</con_control><con_exclusive>0</con_exclusive><c_name>Desk</c_name></device></devices>"))
        }
        "connect_channel" => {
            *connected.lock().unwrap() = Some(get("c_id"));
            ok("")
        }
        _ => ok("<total_devices>0</total_devices><count_devices>0</count_devices>"),
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_in_reads_receivers_and_connects_a_channel() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connected = Arc::new(Mutex::new(None));
    let c = connected.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let c = c.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    // One request at a time; a keep-alive connection may carry more.
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = stream.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                    let head = String::from_utf8_lossy(&buf[..end]).to_string();
                    buf.drain(..end);
                    let target = head.split_whitespace().nth(1).unwrap_or("").to_string();
                    let query = target.split_once('?').map_or("", |(_, q)| q).to_string();
                    let body = answer(&query, &c);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if stream.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "adder-aim".into(),
            model: "aim".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(
                json!({"username": "admin", "password": "password", "poll_interval_ms": 500}),
            ),
            monitor: true,
        })
        .unwrap();

    let wait = |check: fn(&Value) -> bool| {
        let core = &core;
        async move {
            tokio::time::timeout(Duration::from_secs(15), async {
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
    let state = wait(|s| s["receivers"]["170"]["name"] == json!("RX 123")).await;
    assert_eq!(state["receivers"]["170"]["connected"], false);

    let outcome = core
        .execute(
            device,
            "connect_channel",
            params(json!({"receiver": 170, "channel": 4})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Value { value: json!(true) }));
    let state = wait(|s| s["receivers"]["170"]["channel"] == json!("4")).await;
    assert_eq!(state["receivers"]["170"]["mode"], "shared");
    core.close(device).await;
}
