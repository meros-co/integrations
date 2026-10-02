//! AJA's event connection (the aja-config-events extension) against a KUMO
//! router simulated on a real HTTP socket, driven through the public API.
//!
//! The router answers `/config?action=connect` with a connection id and holds
//! each `wait_for_config_events` until it has a change: the first wait on
//! connection 1 returns a crosspoint change after a delay, the next one finds
//! the id expired; the extension connects again and gets connection 2, whose
//! first wait returns another change and whose later waits are held and
//! return nothing. Every `/config?action=get` is answered, so the engine's
//! reads on connecting and commands work beside the waits.

#![cfg(all(feature = "aja-kipro", feature = "aja-kumo"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meros_integrations::{Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
struct Router {
    connects: AtomicUsize,
    waits_on_1: AtomicUsize,
    waits_on_2: AtomicUsize,
}

async fn read_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    head.lines().next()?.split(' ').nth(1).map(str::to_string)
}

async fn answer(router: &Router, target: &str) -> (&'static str, String) {
    if target == "/config?action=connect" {
        let n = router.connects.fetch_add(1, Ordering::SeqCst) + 1;
        return (
            "200 OK",
            json!({ "connectionid": n.to_string() }).to_string(),
        );
    }
    if let Some(id) =
        target.strip_prefix("/config?action=wait_for_config_events&configid=0&connectionid=")
    {
        return match id {
            "1" => match router.waits_on_1.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    // Held while the consumer runs a command.
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    (
                        "200 OK",
                        r#"[{"param_id":"eParamID_XPT_Destination3_Status","int_value":7,"str_value":""},
                            {"param_id":"eParamID_XPT_Source2_Line_1","int_value":0,"str_value":"CAM 2"}]"#
                            .into(),
                    )
                }
                // Expired.
                _ => ("200 OK", r#"{"error":"invalid connection id"}"#.into()),
            },
            "2" => match router.waits_on_2.fetch_add(1, Ordering::SeqCst) {
                0 => (
                    "200 OK",
                    r#"[{"param_id":"eParamID_XPT_Destination4_Status","int_value":9,"str_value":""}]"#
                        .into(),
                ),
                _ => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    ("200 OK", "[]".into())
                }
            },
            _ => ("404 Not Found", "no such connection".into()),
        };
    }
    if let Some(param) = target.strip_prefix("/config?action=get&paramid=") {
        let value = if param == "eParamID_XPT_Destination1_Status" {
            "16"
        } else {
            "1"
        };
        return (
            "200 OK",
            json!({"paramid": "1", "name": param, "value": value, "value_name": value}).to_string(),
        );
    }
    ("200 OK", "{}".into())
}

async fn simulate() -> (u16, Arc<Router>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Arc::new(Router::default());
    let shared = router.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let router = shared.clone();
            tokio::spawn(async move {
                let Some(target) = read_target(&mut stream).await else {
                    return;
                };
                let (status, body) = answer(&router, &target).await;
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (port, router)
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("state within 10 s: {}", core.snapshot(id).unwrap().state))
}

#[tokio::test(flavor = "multi_thread")]
async fn kumo_events_connect_wait_and_expire() {
    let (port, router) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "aja-kumo".into(),
            model: "kumo-1616".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
        })
        .unwrap();

    // Connected, and the first wait is being held.
    let held = Instant::now();
    tokio::time::timeout(Duration::from_secs(5), async {
        while router.waits_on_1.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the extension connects and waits");

    // A command goes out and is answered while that wait is held.
    let route = core
        .execute(
            id,
            "get_route",
            json!({"destination": 1}).as_object().unwrap().clone(),
        )
        .await;
    assert_eq!(route, Ok(Outcome::Value { value: json!("16") }));
    assert!(
        held.elapsed() < Duration::from_millis(1400),
        "the command waited for the event connection"
    );
    assert_ne!(core.snapshot(id).unwrap().state["outputs"]["3"]["input"], 7);

    // The held wait's change becomes state through the spec's rules.
    wait_for_state(&core, id, |s| s["outputs"]["3"]["input"] == 7).await;
    let st = core.snapshot(id).unwrap().state;
    assert_eq!(st["inputs"]["2"]["label_line_1"], "CAM 2");
    assert_eq!(
        st["params"]["eParamID_XPT_Destination3_Status"],
        json!({"value": "7", "value_name": "7"})
    );

    // The next wait finds the id expired: a new connection, and its change.
    wait_for_state(&core, id, |s| s["outputs"]["4"]["input"] == 9).await;
    assert_eq!(router.connects.load(Ordering::SeqCst), 2);
    assert_eq!(router.waits_on_1.load(Ordering::SeqCst), 2);
    core.close(id).await;
}
