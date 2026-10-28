//! PTP monitoring against real loopback sockets, through the public API, by
//! unicast so nothing leaves the host: a grandmaster simulated as a UDP
//! socket sends Announce on the general port and one-step Sync on the event
//! port (PTP version 2 layouts), and the core keeps the grandmaster and the
//! port.

#![cfg(feature = "ptp")]

mod common;

use std::time::Duration;

use meros_integrations::OpenRequest;
use serde_json::{json, Value};
use tokio::net::UdpSocket;

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

const GM: [u8; 8] = [0x00, 0x1D, 0xC1, 0xFF, 0xFE, 0x0A, 0x1B, 0x2C];

fn message(kind: u8, flags: u16, seq: u16, body: &[u8]) -> Vec<u8> {
    let mut m = vec![kind, 0x02];
    m.extend(((34 + body.len()) as u16).to_be_bytes());
    m.extend([127, 0]);
    m.extend(flags.to_be_bytes());
    m.extend([0; 12]);
    m.extend(GM);
    m.extend(1u16.to_be_bytes());
    m.extend(seq.to_be_bytes());
    m.extend([0, 0xFE]);
    m.extend(body);
    m
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_grandmaster_and_its_sync() {
    let core = common::core().unwrap();
    let event = free_port();
    let general = free_port();
    let device = core
        .open(OpenRequest {
            device: "ptp".into(),
            model: "ptp".into(),
            host: String::new(),
            port: None,
            settings: params(json!({
                "bind_address": "127.0.0.1",
                "event_port": event,
                "general_port": general,
                "report_interval_ms": 0,
            })),
            monitor: true,
        })
        .unwrap();

    let gm = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut announce = vec![0; 10];
    announce.extend(37i16.to_be_bytes());
    announce.extend([0, 128, 6, 0x21, 0x4E, 0x5D, 128]);
    announce.extend(GM);
    announce.extend([0, 0, 0x20]);
    let announce = message(0xB, 0x003C, 1, &announce);
    let sync = message(0x0, 0, 1, &[0, 0, 0x65, 0x53, 0xF1, 0x25, 0, 0, 0, 0]);
    let key = "00-1D-C1-FF-FE-0A-1B-2C:1";
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            gm.send_to(&announce, ("127.0.0.1", general)).await.unwrap();
            gm.send_to(&sync, ("127.0.0.1", event)).await.unwrap();
            let state = core.snapshot(device).unwrap().state;
            let domain = &state["domains"]["127"];
            if domain["grandmaster"]["clock_class"] == json!(6)
                && domain["ports"][key]["sync"]["origin"].is_string()
            {
                assert_eq!(domain["best_master"], key);
                assert_eq!(domain["grandmaster"]["identity"], "00-1D-C1-FF-FE-0A-1B-2C");
                assert_eq!(domain["ports"][key]["role"], "master");
                assert_eq!(domain["ports"][key]["sync"]["two_step"], false);
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the grandmaster is kept");
    core.close(device).await;
}
