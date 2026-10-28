//! An NTP server simulated on real UDP, driven through the public API. The
//! simulator answers each client request (RFC 5905 §7.3, mode 3) with a
//! stratum 1 server packet whose originate timestamp is the request's
//! transmit timestamp and whose receive and transmit times are this host's
//! clock.

#![cfg(feature = "generic-ntp")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use meros_integrations::{OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

fn now_ntp() -> u64 {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = d.as_secs() + 2_208_988_800;
    let frac = (u64::from(d.subsec_nanos()) << 32) / 1_000_000_000;
    (secs << 32) | frac
}

async fn simulate() -> (u16, Arc<Mutex<usize>>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let count = Arc::new(Mutex::new(0));
    let c = count.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 128];
        while let Ok((n, from)) = socket.recv_from(&mut buf).await {
            assert_eq!(n, 48);
            assert_eq!(buf[0] & 0x07, 3, "a client request");
            *c.lock().unwrap() += 1;
            let mut p = [0u8; 48];
            p[0] = 0x24;
            p[1] = 1;
            p[2] = 6;
            p[3] = 0xEC;
            p[12..16].copy_from_slice(b"PPS\0");
            let t = now_ntp();
            p[16..24].copy_from_slice(&t.to_be_bytes());
            p[24..32].copy_from_slice(&buf[40..48]);
            p[32..40].copy_from_slice(&t.to_be_bytes());
            p[40..48].copy_from_slice(&now_ntp().to_be_bytes());
            let _ = socket.send_to(&p, from).await;
        }
    });
    (port, count)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_is_read_and_get_status_returns_it() {
    let (port, count) = simulate().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "generic-ntp".into(),
            model: "ntp-server".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
            monitor: true,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.snapshot(id).unwrap().state["stratum"] == 1 {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s");
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["reference_id"], json!("PPS"));
    assert_eq!(state["synchronized"], json!(true));
    // The same host's clock on both sides: the offset is small.
    assert!(state["offset_ms"].as_f64().unwrap().abs() < 1000.0);
    match core.execute(id, "get_status", params(json!({}))).await {
        Ok(Outcome::Value { value }) => assert_eq!(value["stratum"], json!(1)),
        other => panic!("{other:?}"),
    }
    assert!(*count.lock().unwrap() >= 2);
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    core.close(id).await;
}
