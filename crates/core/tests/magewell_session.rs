//! Magewell Pro Convert devices simulated on real TCP, driven through the
//! public API: the spec's cookie session (`transport.session`) end to end.
//!
//! The simulator answers each HTTP request as the device's web server does:
//! login sets a session cookie, every other method but ping needs it, and a
//! session can be ended to see the core log in again.

#![cfg(feature = "magewell-proconvert")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Default)]
struct Device {
    /// Every request, its head and body as received.
    requests: Vec<String>,
    /// The session the device accepts; `None` once ended.
    session: Option<String>,
    logins: usize,
    /// Answer logins with status 36, a wrong password.
    refuse: bool,
}

type Shared = Arc<Mutex<Device>>;

/// One request from a connection, head and body.
async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let length = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while buf.len() < head_end + 4 + length {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Some(String::from_utf8_lossy(&buf).to_string())
}

/// The cookie header's value, whatever the header's case.
fn cookie(request: &str) -> Option<String> {
    request
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("cookie"))
        .map(|(_, v)| v.trim().to_string())
}

async fn simulated(shared: Shared, ip_decoder: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let shared = shared.clone();
            tokio::spawn(async move {
                while let Some(request) = read_request(&mut stream).await {
                    let line = request.lines().next().unwrap_or("").to_string();
                    let (set_cookie, body) = {
                        let mut d = shared.lock().unwrap();
                        d.requests.push(request.clone());
                        let login =
                            line.contains("method=login") || line.contains("/api/user/login");
                        let ping = line.contains("method=ping") || line.contains("/api/ping");
                        let authed = d.session.is_some() && cookie(&request) == d.session;
                        if login && d.refuse {
                            (String::new(), json!({"status": 36}))
                        } else if login {
                            d.logins += 1;
                            let name = if ip_decoder {
                                "sid-A506220808450"
                            } else {
                                "sid"
                            };
                            let value = format!("{name}=s{}", d.logins);
                            d.session = Some(value.clone());
                            (
                                format!("Set-Cookie: {value}; path=/\r\n"),
                                json!({"status": 0}),
                            )
                        } else if ping {
                            (String::new(), json!({"status": 0}))
                        } else if !authed {
                            (String::new(), json!({"status": 37}))
                        } else if line.contains("method=get-summary-info") {
                            (
                                String::new(),
                                json!({"status": 0, "device": {"model": "NDI to HDMI", "fw-version": "1.3.100"},
                                       "ndi": {"name": "STUDIO (Camera 1)", "tally-program": true,
                                               "tally-preview": false, "video-width": 1920,
                                               "video-height": 1080, "video-scan": "progressive",
                                               "video-field-rate": 59.94}}),
                            )
                        } else if line.contains("/api/system/summary") {
                            (
                                String::new(),
                                json!({"status": 0, "product-name": "Pro Convert IP to HDMI", "hdmi-state": 1,
                                       "profile": {"streams": [{"name": "CAM (1)", "video": {"kbps": 8000},
                                                                "extra": {"tally-program": true}}]}}),
                            )
                        } else if line.contains("/api/ptz/preset/recall") {
                            (String::new(), json!({"status": 7, "code": "Invalid"}))
                        } else {
                            (String::new(), json!({"status": 0}))
                        }
                    };
                    let body = body.to_string();
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\n{set_cookie}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if stream.write_all(reply.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
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

fn open(core: &Core, model: &str, port: u16, monitor: bool) -> u64 {
    core.open(OpenRequest {
        device: "magewell-proconvert".into(),
        model: model.into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({})),
        monitor,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decoder_logs_in_polls_and_logs_in_again_when_the_session_ends() {
    let shared = Shared::default();
    let port = simulated(shared.clone(), false).await;
    let core = Core::new().unwrap();
    let id = open(&core, "ndi-to-hdmi", port, true);

    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["ndi"]["name"] == "STUDIO (Camera 1)" && patch["tally"]["program"] == true
            && patch["ndi"]["video"] == "1920x1080 progressive 59.94")
    })
    .await;
    {
        let d = shared.lock().unwrap();
        // MD5("Admin"), the Encoder API's example.
        assert!(d.requests[0].starts_with(
            "GET /mwapi?method=login&id=Admin&pass=e3afed0047b08059d0fada10f400c1e5 "
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_logs_in_again_when_the_session_has_ended() {
    let shared = Shared::default();
    let port = simulated(shared.clone(), false).await;
    let core = Core::new().unwrap();
    // Commands only, so no poll renews the session meanwhile.
    let id = open(&core, "ndi-to-hdmi", port, false);
    assert!(core
        .execute(id, "get_caps", params(json!({})))
        .await
        .is_ok());
    assert_eq!(shared.lock().unwrap().logins, 1);

    // The device restarts: the session is gone, and the next request is
    // answered "not logged in" (37). The core logs in and sends it again.
    shared.lock().unwrap().session = None;
    let outcome = core
        .execute(
            id,
            "select_ndi_source",
            params(json!({"name": "STUDIO (Camera 2)"})),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    let d = shared.lock().unwrap();
    assert_eq!(d.logins, 2);
    let sent: Vec<&String> = d
        .requests
        .iter()
        .filter(|r| {
            r.starts_with(
                "GET /mwapi?method=set-channel&ndi-name=true&name=STUDIO%20%28Camera%202%29 ",
            )
        })
        .collect();
    assert_eq!(sent.len(), 2, "sent once, then once more after the login");
    assert_eq!(cookie(sent[1]), d.session);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ip_decoder_logs_in_with_sha256_json() {
    let shared = Shared::default();
    let port = simulated(shared.clone(), true).await;
    let core = Core::new().unwrap();
    let id = open(&core, "ip-to-hdmi", port, true);
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["ndi"]["name"] == "CAM (1)" && patch["device"]["output_state"] == "1")
    })
    .await;
    {
        let d = shared.lock().unwrap();
        assert!(d.requests[0].starts_with("POST /api/user/login "));
        // SHA-256("Admin"), the IP Decoder API's example.
        assert!(d.requests[0].ends_with(
            r#"{"username":"Admin","password":"c1c224b03cd9bc7b6a86d77f5dace40191766c485cd55dc48caf9ac873335d6f"}"#
        ));
    }
    // A failing status is the command's error, with its code.
    let outcome = core
        .execute(id, "ptz_recall_preset", params(json!({"number": 3})))
        .await;
    assert!(
        matches!(&outcome, Err(CommandError::DeviceError { code: Some(c), .. }) if c == "7"),
        "{outcome:?}"
    );
    let d = shared.lock().unwrap();
    let recall = d
        .requests
        .iter()
        .find(|r| r.starts_with("POST /api/ptz/preset/recall "))
        .unwrap();
    assert_eq!(cookie(recall).as_deref(), Some("sid-A506220808450=s1"));
    assert!(recall.ends_with(r#"{"number":3}"#));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_login_is_final_and_commands_only_pings_without_one() {
    let shared = Shared::default();
    shared.lock().unwrap().refuse = true;
    let port = simulated(shared.clone(), false).await;
    let core = Core::new().unwrap();

    // Opened for commands only: ping needs no login.
    let quiet = open(&core, "sdi-plus", port, false);
    assert_eq!(
        core.execute(quiet, "ping", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    assert!(shared
        .lock()
        .unwrap()
        .requests
        .iter()
        .all(|r| !r.contains("method=login") && cookie(r).is_none()));

    // A command needing a session meets the refused login: final.
    let outcome = core.execute(quiet, "reboot", params(json!({}))).await;
    assert!(
        matches!(outcome, Err(CommandError::Auth { .. })),
        "{outcome:?}"
    );
    assert!(matches!(
        core.snapshot(quiet).unwrap().connection,
        Connection::Unauthorized { .. }
    ));
    let outcome = core.execute(quiet, "get_caps", params(json!({}))).await;
    assert!(
        matches!(outcome, Err(CommandError::Auth { .. })),
        "{outcome:?}"
    );
    let logins = shared
        .lock()
        .unwrap()
        .requests
        .iter()
        .filter(|r| r.contains("method=login"))
        .count();
    assert_eq!(logins, 1);
}
