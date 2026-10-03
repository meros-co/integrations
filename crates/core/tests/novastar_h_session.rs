//! A NovaStar H series splicer's OpenAPI simulated on real HTTP, driven through
//! the public API.
//!
//! The simulator checks every request as the H Series OpenAPI instructions
//! define it: a POST of {"body", "sign", "pId", "timeStamp"} whose sign is
//! Base64 of the hexadecimal MD5 of timeStamp + pId, and a timeStamp within a
//! minute of its own clock (it answers status 11, incorrect device time,
//! otherwise). It serves the initialisation status, a screen list, the
//! current preset, and loads presets.

#![cfg(feature = "novastar-h")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use md5::{Digest, Md5};
use meros_integrations::{CommandError, Core, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const P_ID: &str = "YmRj";

struct Splicer {
    preset: Mutex<i64>,
    paths: Mutex<Vec<String>>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl Splicer {
    fn answer(&self, path: &str, request: &Value) -> Value {
        let time = request["timeStamp"].as_str().unwrap_or("");
        let hex: String = Md5::digest(format!("{time}{P_ID}").as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected = base64::engine::general_purpose::STANDARD.encode(hex);
        if request["sign"] != json!(expected) || request["pId"] != json!(P_ID) {
            return json!({"status": 18, "msg": "Incorrect signature.", "body": ""});
        }
        let t: i64 = time.parse().unwrap_or(0);
        if (t - now_ms()).abs() > 60_000 {
            return json!({"status": 11, "msg": "Incorrect device time.", "body": ""});
        }
        let body = &request["body"];
        match path {
            "/open/api/main/initStatus" => json!({"status": 0, "msg": "Success", "sign": "",
                "body": {"initStatus": 1, "mainModelId": 29962, "sn": "FFFFFFFFFFFFFFFF", "softwareVersion": "1.9.7.0.S1.T1"}}),
            "/open/api/screen/readList" => json!({"status": 0, "msg": "", "sign": "",
                "body": {"deviceId": 0, "screens": [{"createTime": "1757509392.7567859", "name": "Screen 1", "screenId": 0}]}}),
            "/open/api/preset/readPlay" => json!({"status": 0, "msg": "Success", "sign": "",
                "body": {"deviceId": 0, "presetId": *self.preset.lock().unwrap(), "screenId": 0}}),
            "/open/api/preset/play" => {
                if body["screenId"] != json!(0) {
                    return json!({"status": 404, "msg": "The screen does not exist.", "body": ""});
                }
                *self.preset.lock().unwrap() = body["presetId"].as_i64().unwrap_or(-1);
                json!({"status": 0, "msg": "Success", "sign": "", "body": ""})
            }
            _ => json!({"status": 94, "msg": "Invalid command.", "body": ""}),
        }
    }

    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                let Ok(n) = stream.read(&mut chunk).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
                continue;
            };
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            while buf.len() < end + 4 + length {
                let Ok(n) = stream.read(&mut chunk).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let body: Vec<u8> = buf.drain(..end + 4 + length).skip(end + 4).collect();
            let mut first = head.lines().next().unwrap_or("").split_whitespace();
            let method = first.next().unwrap_or("");
            let path = first.next().unwrap_or("").to_string();
            assert_eq!(method, "POST");
            self.paths.lock().unwrap().push(path.clone());
            let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let reply = self.answer(&path, &request).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            );
            if stream.write_all(response.as_bytes()).await.is_err() {
                return;
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Splicer>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let splicer = Arc::new(Splicer {
        preset: Mutex::new(-1),
        paths: Mutex::default(),
    });
    let s = splicer.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(s.clone().serve(stream));
        }
    });
    (port, splicer)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
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
    .expect("state within 10 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_requests_poll_the_screens_and_load_presets() {
    let (port, splicer) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "novastar-h".into(),
            model: "h5".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"p_id": P_ID, "poll_ms": 200})),
            monitor: true,
        })
        .unwrap();

    wait_for_state(&core, id, |s| {
        s["device"]["initialized"] == true
            && s["device"]["software_version"] == "1.9.7.0.S1.T1"
            && s["screens"]["0"]["name"] == "Screen 1"
            && s["screens"]["0"]["preset"] == -1
    })
    .await;

    assert_eq!(
        core.execute(
            id,
            "load_preset",
            params(json!({"screen_id": 0, "preset_id": 2}))
        )
        .await,
        Ok(Outcome::Value { value: json!("") })
    );
    wait_for_state(&core, id, |s| s["screens"]["0"]["preset"] == 2).await;

    match core
        .execute(
            id,
            "load_preset",
            params(json!({"screen_id": 5, "preset_id": 2})),
        )
        .await
    {
        Err(CommandError::DeviceError { code, .. }) => assert_eq!(code.as_deref(), Some("404")),
        other => panic!("{other:?}"),
    }
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());
    assert!(splicer
        .paths
        .lock()
        .unwrap()
        .iter()
        .all(|p| p.starts_with("/open/api/")));
}
