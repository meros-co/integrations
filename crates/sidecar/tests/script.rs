//! Plays tests/bindings/script.json through the sidecar over HTTP against
//! integrations-sim, the same script every other delivery runs.

// The script drives devices from several integrations, so it needs them all.
#![cfg(feature = "all")]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn first_line(child: &mut Child) -> Value {
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    serde_json::from_str(&line).unwrap()
}

fn strip_messages(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| k.as_str() != "message")
                .map(|(k, v)| (k.clone(), strip_messages(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(strip_messages).collect()),
        other => other.clone(),
    }
}

fn substitute(v: &Value, vars: &Map<String, Value>) -> Value {
    match v {
        Value::String(s) if s.starts_with('$') => vars[&s[1..]].clone(),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), substitute(v, vars)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| substitute(v, vars)).collect()),
        other => other.clone(),
    }
}

fn at<'a>(v: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(v, |node, key| &node[key])
}

/// Kills the child when dropped, so a failed assertion does not leave a
/// sidecar running and holding the binary open.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    async fn get(&self, path: &str) -> Value {
        self.http
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> Value {
        self.http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read `count` parts of a `multipart/x-mixed-replace` JPEG stream, then
/// hang up: `(X-Sequence, body)` for each.
async fn read_mjpeg(client: &Client, path: &str, count: usize) -> Vec<(u64, Vec<u8>)> {
    let mut response = client
        .http
        .get(format!("{}{path}", client.base))
        .bearer_auth(&client.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let content_type = response.headers()["content-type"].to_str().unwrap();
    let boundary = content_type
        .strip_prefix("multipart/x-mixed-replace; boundary=")
        .expect("multipart/x-mixed-replace")
        .to_string();
    let mut buf = Vec::new();
    let mut parts = Vec::new();
    while parts.len() < count {
        let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk())
            .await
            .expect("a frame within 5 s")
            .unwrap()
            .expect("the stream stays open");
        buf.extend_from_slice(&chunk);
        while let Some(end) = find(&buf, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let mut lines = head.split("\r\n");
            assert_eq!(lines.next(), Some(format!("--{boundary}").as_str()));
            let mut length = 0;
            let mut sequence = 0;
            for line in lines {
                let (name, value) = line.split_once(": ").unwrap();
                match name.to_ascii_lowercase().as_str() {
                    "content-length" => length = value.parse().unwrap(),
                    "x-sequence" => sequence = value.parse().unwrap(),
                    "content-type" => assert_eq!(value, "image/jpeg"),
                    _ => {}
                }
            }
            let body_start = end + 4;
            if buf.len() < body_start + length + 2 {
                break;
            }
            parts.push((sequence, buf[body_start..body_start + length].to_vec()));
            assert_eq!(&buf[body_start + length..body_start + length + 2], b"\r\n");
            buf.drain(..body_start + length + 2);
        }
    }
    parts.truncate(count);
    parts
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shared_binding_script() {
    let status = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "meros-integrations-sim"])
        .current_dir(root())
        .status()
        .unwrap();
    assert!(status.success());
    let sim_exe = root()
        .join("target/debug")
        .join(format!("integrations-sim{}", std::env::consts::EXE_SUFFIX));
    let mut sim = Command::new(sim_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut vars = first_line(&mut sim).as_object().unwrap().clone();

    let token_file = std::env::temp_dir().join(format!(
        "meros-integrations-test-{}.token",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&token_file);
    let mut sidecar = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
            .args(["serve", "--listen", "127.0.0.1:0", "--token-file"])
            .arg(&token_file)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let listening = first_line(&mut sidecar.0)["listening"]
        .as_str()
        .unwrap()
        .to_string();
    let client = Client {
        base: format!("http://{listening}"),
        token: std::fs::read_to_string(&token_file).unwrap(),
        http: reqwest::Client::new(),
    };

    // Without the token, nothing.
    let refused = client
        .http
        .get(format!("{}/v1/catalog", client.base))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 401);

    let script: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/bindings/script.json")).unwrap(),
    )
    .unwrap();
    for (index, raw) in script["steps"].as_array().unwrap().iter().enumerate() {
        let step = substitute(raw, &vars);
        let label = format!("step {index}: {raw}");
        let device = step.get("device").cloned().unwrap_or(Value::Null);
        match step["op"].as_str().unwrap() {
            "catalog" => {
                let catalog = client.get("/v1/catalog").await;
                assert_eq!(
                    at(&catalog, step["path"].as_str().unwrap()),
                    &step["expect"],
                    "{label}"
                );
            }
            "open" => {
                let result = client.post("/v1/open", step["request"].clone()).await;
                if let Some(expect) = step.get("expect") {
                    assert_eq!(&strip_messages(&result), expect, "{label}");
                }
                if let Some(name) = step.get("save").and_then(Value::as_str) {
                    vars.insert(name.into(), result["device"].clone());
                }
            }
            "wait" => {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let snap = client.get(&format!("/v1/snapshot/{device}")).await;
                    if at(&snap, step["path"].as_str().unwrap()) == &step["equals"] {
                        break;
                    }
                    assert!(Instant::now() < deadline, "{label}: timed out");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
            "execute" => {
                let body =
                    json!({"device": device, "command": step["command"], "params": step["params"]});
                let result = client.post("/v1/execute", body).await;
                assert_eq!(strip_messages(&result), step["expect"], "{label}");
            }
            "stream" => {
                let stream = step["stream"].as_str().unwrap();
                let path = format!("/v1/devices/{device}/streams/{stream}");
                if let Some(expect) = step.get("expect") {
                    let refused = client
                        .http
                        .get(format!("{}{path}.jpg", client.base))
                        .bearer_auth(&client.token)
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(refused.status(), 404, "{label}");
                    let body: Value = refused.json().await.unwrap();
                    assert_eq!(&strip_messages(&body), expect, "{label}");
                    continue;
                }
                assert_eq!(step["format"], "jpeg", "{label}: the sidecar serves JPEG");
                let frames = step["frames"].as_u64().unwrap() as usize;
                let parts = read_mjpeg(&client, &format!("{path}.mjpg"), frames).await;
                let mut last = 0;
                for (sequence, data) in parts {
                    assert!(data.starts_with(&[0xFF, 0xD8]), "{label}");
                    assert!(sequence > last, "{label}");
                    last = sequence;
                }

                // One frame, with the token in the query as an <img> sends it.
                let url = format!("{}{path}.jpg?access_token={}", client.base, client.token);
                let one = client.http.get(&url).send().await.unwrap();
                assert_eq!(one.status(), 200, "{label}");
                assert_eq!(one.headers()["content-type"], "image/jpeg");
                assert!(one.headers().contains_key("x-sequence"));
                assert!(one.bytes().await.unwrap().starts_with(&[0xFF, 0xD8]));
                // Without the token, nothing.
                let url = format!("{}{path}.mjpg?access_token=wrong", client.base);
                assert_eq!(client.http.get(&url).send().await.unwrap().status(), 401);
            }
            "close" => {
                client.post("/v1/close", json!({"device": device})).await;
            }
            other => panic!("unknown op {other}"),
        }
    }

    // Events were queued throughout; the long-poll hands them over.
    let events = client.get("/v1/events?max=1000&wait_ms=100").await;
    assert!(events
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["event"] == "connection"));

    drop(sidecar);
    drop(sim.stdin.take());
    let _ = sim.wait();
    let _ = std::fs::remove_file(&token_file);
}

/// `--devices`: a sidecar for some devices only.
#[tokio::test(flavor = "multi_thread")]
async fn a_sidecar_started_for_some_devices() {
    let token_file = std::env::temp_dir().join(format!(
        "meros-integrations-test-devices-{}.token",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&token_file);
    let mut sidecar = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
            .args(["serve", "--listen", "127.0.0.1:0", "--token-file"])
            .arg(&token_file)
            .args(["--devices", "sennheiser-ew-g3-g4,shure-wireless"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let listening = first_line(&mut sidecar.0)["listening"]
        .as_str()
        .unwrap()
        .to_string();
    let client = Client {
        base: format!("http://{listening}"),
        token: std::fs::read_to_string(&token_file).unwrap(),
        http: reqwest::Client::new(),
    };
    let catalog = client.get("/v1/catalog").await;
    let ids: Vec<&String> = catalog["devices"].as_object().unwrap().keys().collect();
    assert_eq!(ids, ["sennheiser-ew-g3-g4", "shure-wireless"]);
    let refused = client
        .post(
            "/v1/open",
            json!({"device": "kramer-p3000", "model": "p3000-generic", "host": "127.0.0.1"}),
        )
        .await;
    assert_eq!(refused["error"]["error"], "not_selected");
    let refused = client
        .post(
            "/v1/discover",
            json!({"action": "scan", "protocols": ["ssdp"]}),
        )
        .await;
    assert_eq!(refused["error"]["error"], "invalid_request");
    drop(sidecar);
    let _ = std::fs::remove_file(&token_file);

    // An id the build does not have stops it at startup.
    let status = Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
        .args(["serve", "--listen", "127.0.0.1:0", "--token-file"])
        .arg(&token_file)
        .args(["--devices", "no-such-device"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
    let _ = std::fs::remove_file(&token_file);
}

/// SIGTERM, systemd's stop, closes every device before the service exits:
/// a Shure receiver's metering is turned off on the wire.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn sigterm_closes_every_device_before_exiting() {
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // A Shure ULX-D that records what it receives.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let device_port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(String::new()));
    let log = received.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            log.lock().unwrap().push_str(&text);
            if text.contains("< GET 1 ALL >") {
                let _ = stream.write_all(b"< REP 1 CHAN_NAME {Pulpit} >").await;
            }
        }
    });

    let token_file = std::env::temp_dir().join(format!(
        "meros-integrations-test-sigterm-{}.token",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&token_file);
    let mut sidecar = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
            .args(["serve", "--listen", "127.0.0.1:0", "--token-file"])
            .arg(&token_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let listening = first_line(&mut sidecar.0)["listening"]
        .as_str()
        .unwrap()
        .to_string();
    let client = Client {
        base: format!("http://{listening}"),
        token: std::fs::read_to_string(&token_file).unwrap(),
        http: reqwest::Client::new(),
    };
    let opened = client
        .post(
            "/v1/open",
            json!({"device": "shure-wireless", "model": "ulxd4",
                   "host": "127.0.0.1", "port": device_port}),
        )
        .await;
    assert!(opened["device"].is_u64(), "{opened}");
    let started = Instant::now();
    while !received.lock().unwrap().contains("METER_RATE 01000") {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "metering never started"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let status = Command::new("kill")
        .args(["-TERM", &sidecar.0.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let started = Instant::now();
    let exit = loop {
        if let Some(exit) = sidecar.0.try_wait().unwrap() {
            break exit;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "still running after SIGTERM"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(exit.success(), "{exit:?}");
    assert!(received
        .lock()
        .unwrap()
        .contains("< SET 1 METER_RATE 00000 >"));
    let _ = std::fs::remove_file(&token_file);
}
