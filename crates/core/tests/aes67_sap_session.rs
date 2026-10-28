//! AES67 stream discovery against real loopback sockets, through the public
//! API: an announcer simulated as a UDP sender of SAP packets (RFC 2974
//! section 6) carrying Audinate's sample description. The core is bound to
//! loopback, where no group is joined, so announcements are sent to the
//! listening address directly.

#![cfg(feature = "aes67-sap")]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{Connection, Core, DeviceId, OpenRequest};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const SAMPLE: &str = "v=0\r\no=- 123456 123458 IN IP4 10.0.1.2\r\ns=My sample flow\r\n\
    i=4 channels: c1, c2, c3, c4\r\nt=0 0\r\na=recvonly\r\nm=audio 5004 RTP/AVP 98\r\n\
    c=IN IP4 239.69.11.44/32\r\na=rtpmap:98 L24/48000/4\r\na=ptime:1\r\n\
    a=ts-refclk:ptp=IEEE1588-2008:00-11-22-FF-FE-33-44-55:0\r\na=mediaclk:direct=0\r\n";

fn sap(deletion: bool, hash: u16, payload: &str) -> Vec<u8> {
    let mut out = vec![if deletion { 0x24 } else { 0x20 }, 0];
    out.extend_from_slice(&hash.to_be_bytes());
    out.extend_from_slice(&[10, 0, 1, 2]);
    out.extend_from_slice(b"application/sdp\0");
    out.extend_from_slice(payload.as_bytes());
    out
}

/// A UDP port free on 127.0.0.1. Loopback only: see `common`.
fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn open(core: &Core, port: u16) -> DeviceId {
    core.open(OpenRequest {
        device: "aes67-sap".into(),
        model: "generic".into(),
        host: String::new(),
        port: Some(port),
        settings: json!({}).as_object().unwrap().clone(),
        monitor: true,
    })
    .unwrap()
}

async fn wait_for(core: &Core, device: DeviceId, what: &str, test: impl Fn(&Value) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !test(&core.snapshot(device).unwrap().state) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}: {}", core.snapshot(device).unwrap().state));
}

#[tokio::test(flavor = "multi_thread")]
async fn announced_streams_appear_and_deleted_ones_go() {
    let core = common::core().unwrap();
    let port = free_port();
    let device = open(&core, port);
    tokio::time::timeout(Duration::from_secs(5), async {
        while core.snapshot(device).unwrap().connection != Connection::Connected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("listening");
    let to: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let announcer = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Sent until heard: the listening socket may still be opening.
    let announcement = sap(false, 0x4d2, SAMPLE);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            announcer.send_to(&announcement, to).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            if core.snapshot(device).unwrap().state["stream_count"] == 1 {
                return;
            }
        }
    })
    .await
    .expect("the stream is heard");
    let state = core.snapshot(device).unwrap().state;
    let stream = &state["streams"]["10.0.1.2:123456"];
    assert_eq!(stream["name"], "My sample flow");
    assert_eq!(stream["channels"], 4);
    assert_eq!(stream["sender"], "127.0.0.1");
    assert_eq!(stream["media"][0]["connection"]["address"], "239.69.11.44");
    assert_eq!(
        stream["media"][0]["clocks"][0]["grandmaster"],
        "00-11-22-FF-FE-33-44-55"
    );

    announcer
        .send_to(
            &sap(true, 0x4d2, "o=- 123456 123458 IN IP4 10.0.1.2\r\n"),
            to,
        )
        .await
        .unwrap();
    wait_for(&core, device, "the deletion is heard", |s| {
        s["stream_count"] == 0
    })
    .await;
    assert!(core.snapshot(device).unwrap().state["streams"]
        .get("10.0.1.2:123456")
        .is_none());
}
