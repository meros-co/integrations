//! An HTTP spec reaching a second HTTP server of the same device on another
//! port (SPEC.md §2, Endpoints), over real sockets: MediaMTX's Control API
//! and its playback server.

#![cfg(feature = "mediamtx")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Requests a server received: its port, the request line's target, and
/// whether it carried an Authorization header.
type Seen = Arc<Mutex<Vec<(u16, String, bool)>>>;

/// A loopback TCP listener on a port that is free for UDP too, so the port
/// handed to the core names nothing else on this machine.
async fn listener() -> TcpListener {
    loop {
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok() {
            return tcp;
        }
    }
}

/// An HTTP/1.1 server answering every request with `answer(target)`.
fn serve(listener: TcpListener, seen: Seen, answer: fn(&str) -> (u16, String)) {
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 2048];
                loop {
                    let Ok(n) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    while let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..end]).into_owned();
                        buf.drain(..end + 4);
                        let target = head.split(' ').nth(1).unwrap_or("").to_string();
                        let credentialed = head
                            .lines()
                            .any(|l| l.to_ascii_lowercase().starts_with("authorization:"));
                        seen.lock()
                            .unwrap()
                            .push((port, target.clone(), credentialed));
                        let (status, body) = answer(&target);
                        let reply = format!(
                            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                            body.len()
                        );
                        if stream.write_all(reply.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
}

fn api(target: &str) -> (u16, String) {
    if target.starts_with("/v3/") {
        (200, r#"{"itemCount":0,"pageCount":0,"items":[]}"#.into())
    } else {
        (404, r#"{"status":"error","error":"not found"}"#.into())
    }
}

fn playback(target: &str) -> (u16, String) {
    match target {
        "/list?path=cam1" => (
            200,
            r#"[{"start":"2026-10-06T08:00:00Z","duration":60.5,"url":"http://127.0.0.1/get"}]"#
                .into(),
        ),
        _ => (
            404,
            r#"{"status":"error","error":"no recordings found"}"#.into(),
        ),
    }
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

/// Every connection change of the device within `wait`.
async fn connections(core: &Core, id: u64, wait: Duration) -> Vec<Connection> {
    let mut out = Vec::new();
    let _ = tokio::time::timeout(wait, async {
        loop {
            for event in core.next_events(64).await {
                if let Event::Connection { device, connection } = event {
                    if device == id {
                        out.push(connection);
                    }
                }
            }
        }
    })
    .await;
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn requests_reach_a_second_server_of_the_device_on_its_own_port() {
    let seen: Seen = Arc::default();
    let api_listener = listener().await;
    let playback_listener = listener().await;
    let api_port = api_listener.local_addr().unwrap().port();
    let playback_port = playback_listener.local_addr().unwrap().port();
    serve(api_listener, seen.clone(), api);
    serve(playback_listener, seen.clone(), playback);

    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "mediamtx".into(),
            model: "mediamtx-1-21".into(),
            host: "127.0.0.1".into(),
            // The port given when opening is the Control API's only.
            port: Some(api_port),
            settings: params(json!({
                "auth": "basic", "username": "operator", "password": "pw",
                "playback_port": playback_port,
            })),
            monitor: false,
        })
        .unwrap();
    assert!(connections(&core, id, Duration::from_secs(2))
        .await
        .contains(&Connection::Connected));

    // The playback server answers on its own port, without the API's
    // credential (playback_auth none, the default).
    assert_eq!(
        core.execute(id, "list_playback_spans", params(json!({"path": "cam1"})))
            .await,
        Ok(Outcome::Value {
            value: json!([{"start": "2026-10-06T08:00:00Z", "duration": 60.5, "url": "http://127.0.0.1/get"}])
        })
    );
    // An empty range is that command's 404, nothing more.
    assert!(matches!(
        core.execute(id, "list_playback_spans", params(json!({"path": "cam2"})))
            .await,
        Err(CommandError::DeviceError { code: Some(c), .. }) if c == "404"
    ));
    // The Control API on the transport's port, with the credential.
    assert_eq!(
        core.execute(id, "get_info", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!({"itemCount": 0, "pageCount": 0, "items": []})
        })
    );
    {
        let seen = seen.lock().unwrap();
        assert!(seen.contains(&(playback_port, "/list?path=cam1".into(), false)));
        assert!(seen.contains(&(playback_port, "/list?path=cam2".into(), false)));
        assert!(seen.contains(&(api_port, "/v3/info".into(), true)));
        // The probe never goes to the playback server.
        assert!(seen
            .iter()
            .all(|(port, target, _)| *port == api_port || target.starts_with("/list")));
    }
    core.close(id).await;

    // A playback server that is not running fails its commands, and leaves
    // the device connected through the Control API.
    let closed = listener().await;
    let gone = closed.local_addr().unwrap().port();
    drop(closed);
    let id = core
        .open(OpenRequest {
            device: "mediamtx".into(),
            model: "mediamtx-1-21".into(),
            host: "127.0.0.1".into(),
            port: Some(api_port),
            settings: params(json!({"playback_port": gone})),
            monitor: false,
        })
        .unwrap();
    assert!(connections(&core, id, Duration::from_secs(2))
        .await
        .contains(&Connection::Connected));
    assert!(matches!(
        core.execute(id, "list_playback_spans", params(json!({"path": "cam1"})))
            .await,
        Err(CommandError::Transport { .. })
    ));
    assert_eq!(
        connections(&core, id, Duration::from_millis(500)).await,
        Vec::<Connection>::new()
    );
    assert_eq!(
        core.execute(id, "get_info", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!({"itemCount": 0, "pageCount": 0, "items": []})
        })
    );
    core.close(id).await;
}
