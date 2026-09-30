//! Plays tests/bindings/script.json through the sidecar over HTTP against
//! integrations-sim, the same script every other delivery runs.

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
    let mut sidecar = Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
        .args(["serve", "--listen", "127.0.0.1:0", "--token-file"])
        .arg(&token_file)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let listening = first_line(&mut sidecar)["listening"]
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

    let _ = sidecar.kill();
    drop(sim.stdin.take());
    let _ = sim.wait();
    let _ = std::fs::remove_file(&token_file);
}
