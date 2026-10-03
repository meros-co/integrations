//! Streams through a real session: the HTTP snapshot module against a local
//! HTTP/1.1 server counting requests, driven through the public API.
//!
//! Checks that frames are fetched only while someone watches, that a slow
//! consumer gets the newest frame with the rest counted as dropped, and that
//! closing the device ends every watcher.

#![cfg(all(feature = "generic-http", feature = "http-snapshot"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use meros_integrations::{json as api, Connection, Core, OpenRequest, StreamError};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Serves GET /snapshot.jpg with a JPEG whose third byte counts requests.
async fn camera() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let served = count.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let count = served.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut read = BufReader::new(read);
                loop {
                    let mut line = String::new();
                    if read.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return;
                    }
                    loop {
                        let mut header = String::new();
                        match read.read_line(&mut header).await {
                            Ok(0) | Err(_) => return,
                            Ok(_) if header == "\r\n" => break,
                            Ok(_) => {}
                        }
                    }
                    let n = count.fetch_add(1, Ordering::SeqCst) + 1;
                    let body = [0xFF, 0xD8, n as u8, 0xFF, 0xD9];
                    let mut reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    reply.extend_from_slice(&body);
                    if write.write_all(&reply).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, count)
}

async fn connected(core: &Core, id: u64) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while core.snapshot(id).unwrap().connection != Connection::Connected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connected within 10 s");
}

#[tokio::test(flavor = "multi_thread")]
async fn frames_flow_only_while_watched() {
    let (port, requests) = camera().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "http-snapshot".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: json!({"path": "/snapshot.jpg", "interval_ms": 40})
                .as_object()
                .unwrap()
                .clone(),
            monitor: true,
        })
        .unwrap();
    connected(&core, id).await;

    // The catalogue says what can be watched.
    let streams = &api::catalog(&core)["devices"]["http-snapshot"]["streams"];
    assert_eq!(streams["live"]["format"], "jpeg");

    // Unwatched: the probe only.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let live = core.open_stream(id, "live").unwrap();
    assert_eq!(live.format(), "jpeg");
    let first = tokio::time::timeout(Duration::from_secs(5), live.next_frame())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.format, "jpeg");
    assert!(first.data.starts_with(&[0xFF, 0xD8]));

    // A consumer that falls behind gets the newest frame, with the frames it
    // missed counted, not queued.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let late = live.try_frame().expect("a frame is waiting");
    assert!(late.dropped >= 3, "dropped {}", late.dropped);
    assert_eq!(late.sequence, first.sequence + late.dropped + 1);
    assert_eq!(live.try_frame(), None);

    // A second watcher of a running stream gets a picture at once.
    let second = core.open_stream(id, "live").unwrap();
    assert!(second.try_frame().is_some());
    drop(second);

    // Unwatched again: fetching stops, apart from one already in flight.
    core.close_stream(live);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let stopped_at = requests.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(requests.load(Ordering::SeqCst) <= stopped_at + 1);

    // Errors, shaped for every binding.
    assert_eq!(
        core.open_stream(id, "nope").unwrap_err(),
        StreamError::UnknownStream {
            stream: "nope".into()
        }
    );
    assert_eq!(
        api::open_stream(&core, id, "nope").unwrap_err()["error"]["error"],
        "unknown_stream"
    );

    // Closing the device ends a waiting watcher.
    let live = Arc::new(core.open_stream(id, "live").unwrap());
    tokio::time::timeout(Duration::from_secs(5), live.next_frame())
        .await
        .unwrap()
        .unwrap();
    let waiter = {
        let live = live.clone();
        tokio::spawn(async move {
            // Drain until the end.
            while live.next_frame().await.is_some() {}
        })
    };
    core.close(id).await;
    tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .expect("stream ends when the device closes")
        .unwrap();
    assert!(live.is_closed());
    assert_eq!(
        core.open_stream(id, "live").unwrap_err(),
        StreamError::Closed
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_without_streams_refuse_them() {
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "generic-http".into(),
            model: "generic".into(),
            host: "127.0.0.1".into(),
            port: Some(9),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();
    assert!(matches!(
        core.open_stream(id, "live"),
        Err(StreamError::UnknownStream { .. })
    ));
    core.close(id).await;
}
