//! A CITP/MSEX 1.2 media server simulated on loopback, driven through the
//! public API. The simulator lays its messages out as the CITP protocol
//! suite specification (2020-11-27) does: on a connection it sends PINF/PNam,
//! answers MSEX/CInf with a 1.2 SInf, sends a layer's LSta, answers GELI with
//! one media library holding one element, GEIn with that element's MEIn,
//! GVSr with one video source and GETh with a JPEG thumbnail, and answers
//! anything else with Nack.

#![cfg(feature = "citp-msex")]

mod common;

use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn citp(layer: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut m = b"CITP".to_vec();
    m.extend([1, 0, 0, 0]);
    m.extend(((20 + body.len()) as u32).to_le_bytes());
    m.extend(1u16.to_le_bytes());
    m.extend(0u16.to_le_bytes());
    m.extend(layer);
    m.extend(body);
    m
}

fn msex(cookie: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = vec![1, 2];
    b.extend(cookie);
    b.extend(body);
    citp(b"MSEX", &b)
}

fn ucs2(text: &str) -> Vec<u8> {
    let mut b: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    b.extend([0, 0]);
    b
}

fn answer(cookie: &[u8]) -> Vec<u8> {
    match cookie {
        b"CInf" => {
            let mut b = b"550e8400-e29b-41d4-a716-446655440000".to_vec();
            b.extend(ucs2("Test Server"));
            b.extend([1, 2, 3, 1]);
            b.extend(0x0102u16.to_le_bytes());
            b.extend(1u16.to_le_bytes()); // media only
            b.push(1);
            b.extend(b"JPEG");
            b.push(1);
            b.extend(b"JPEG");
            b.push(1);
            b.extend(b"ArtNet/0/0/1\0");
            let mut out = msex(b"SInf", &b);
            let mut l = vec![1u8, 0, 0, 1, 1, 0, 0, 0, 0];
            l.extend(ucs2("Intro.mov"));
            l.extend(12u32.to_le_bytes());
            l.extend(250u32.to_le_bytes());
            l.push(25);
            l.extend(1u32.to_le_bytes());
            out.extend(msex(b"LSta", &l));
            out
        }
        b"GELI" => {
            let mut b = vec![1u8];
            b.extend(1u16.to_le_bytes());
            b.extend([1, 0, 0, 0]);
            b.extend(1u32.to_le_bytes());
            b.extend([0, 255]);
            b.extend(ucs2("Movies"));
            b.extend(0u16.to_le_bytes());
            b.extend(1u16.to_le_bytes());
            msex(b"ELIn", &b)
        }
        b"GEIn" => {
            let mut b = vec![1u8, 0, 0, 0];
            b.extend(1u16.to_le_bytes());
            b.push(0);
            b.extend(1u32.to_le_bytes());
            b.extend([0, 0]);
            b.extend(ucs2("Intro.mov"));
            b.extend(1_700_000_000u64.to_le_bytes());
            b.extend(1920u16.to_le_bytes());
            b.extend(1080u16.to_le_bytes());
            b.extend(250u32.to_le_bytes());
            b.push(25);
            msex(b"MEIn", &b)
        }
        b"GVSr" => {
            let mut b = 1u16.to_le_bytes().to_vec();
            b.extend(7u16.to_le_bytes());
            b.extend(ucs2("Output 1"));
            b.extend([0, 0xFF]);
            b.extend(0u16.to_le_bytes());
            b.extend(1920u16.to_le_bytes());
            b.extend(1080u16.to_le_bytes());
            msex(b"VSrc", &b)
        }
        b"GETh" => {
            let mut b = vec![1u8, 1, 0, 0, 0, 0];
            b.extend(b"JPEG");
            b.extend(64u16.to_le_bytes());
            b.extend(36u16.to_le_bytes());
            b.extend(4u16.to_le_bytes());
            b.extend([0xFF, 0xD8, 0xFF, 0xD9]);
            msex(b"EThn", &b)
        }
        other => msex(b"Nack", other),
    }
}

async fn serve(stream: tokio::net::TcpStream) {
    let (mut rd, mut wr) = stream.into_split();
    let hello = citp(b"PINF", b"PNamTest Server\0");
    if wr.write_all(&hello).await.is_err() {
        return;
    }
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = rd.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        while buf.len() >= 20 {
            let size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize;
            if buf.len() < size {
                break;
            }
            let message: Vec<u8> = buf.drain(..size).collect();
            if &message[16..20] != b"MSEX" {
                continue;
            }
            if wr.write_all(&answer(&message[22..26])).await.is_err() {
                return;
            }
        }
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_layers_libraries_sources_and_thumbnails_through_the_public_api() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream));
        }
    });

    let core = common::core().unwrap();
    let device = core
        .open(OpenRequest {
            device: "citp-msex".into(),
            model: "media-server".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"client_name": "Test Desk"})),
            monitor: true,
        })
        .unwrap();

    let state = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(device).unwrap().state;
            if state["video_sources"]["7"]["name"] == json!("Output 1")
                && state["libraries"]["media"]["0"]["elements"]["0"]["name"] == json!("Intro.mov")
            {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("state");
    assert_eq!(state["server"]["peer_name"], "Test Server");
    assert_eq!(state["server"]["product"], "Test Server");
    assert_eq!(state["server"]["msex_version"], "1.2");
    assert_eq!(state["layers"]["1"]["dmx_source"], "ArtNet/0/0/1");
    assert_eq!(state["layers"]["1"]["media_name"], "Intro.mov");
    assert_eq!(state["layers"]["1"]["position"], 12);
    assert_eq!(state["layers"]["1"]["playing"], true);
    assert_eq!(state["libraries"]["media"]["0"]["name"], "Movies");
    assert_eq!(
        state["libraries"]["media"]["0"]["elements"]["0"]["width"],
        1920
    );

    let outcome = core
        .execute(
            device,
            "get_element_thumbnail",
            params(json!({"library_type": "media", "library": "0", "element": 0})),
        )
        .await
        .unwrap();
    let Outcome::Value { value } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(value["format"], "jpeg");
    assert_eq!(value["data"], "/9j/2Q==");

    // The server answers a library thumbnail with Nack.
    let refused = core
        .execute(
            device,
            "get_library_thumbnail",
            params(json!({"library_type": "media", "library": "0"})),
        )
        .await;
    assert!(refused.is_err(), "{refused:?}");

    let outcome = core
        .execute(device, "refresh_libraries", params(json!({})))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Ack);
    core.close(device).await;
}
