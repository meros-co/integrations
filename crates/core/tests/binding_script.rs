//! Plays tests/bindings/script.json through the Rust delivery against
//! integrations-sim, the same script every other delivery runs.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use meros_integrations::{json as api, Core};
use serde_json::{Map, Value};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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

#[tokio::test(flavor = "multi_thread")]
async fn the_shared_binding_script() {
    let status = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "meros-integrations-sim"])
        .current_dir(root())
        .status()
        .unwrap();
    assert!(status.success());
    let exe = root()
        .join("target/debug")
        .join(format!("integrations-sim{}", std::env::consts::EXE_SUFFIX));
    let mut sim = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(sim.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let mut vars: Map<String, Value> = serde_json::from_str::<Value>(&line)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();

    let script: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/bindings/script.json")).unwrap(),
    )
    .unwrap();
    let core = Core::new().unwrap();

    for (index, raw) in script["steps"].as_array().unwrap().iter().enumerate() {
        let step = substitute(raw, &vars);
        let label = format!("step {index}: {raw}");
        let device = step.get("device").and_then(Value::as_u64).unwrap_or(0);
        match step["op"].as_str().unwrap() {
            "catalog" => assert_eq!(
                at(&api::catalog(&core), step["path"].as_str().unwrap()),
                &step["expect"],
                "{label}"
            ),
            "open" => {
                let result = api::open(&core, &step["request"]);
                if let Some(expect) = step.get("expect") {
                    assert_eq!(&strip_messages(&result), expect, "{label}");
                }
                if let Some(name) = step.get("save").and_then(Value::as_str) {
                    vars.insert(name.into(), result["device"].clone());
                }
            }
            "wait" => {
                let deadline = Instant::now() + Duration::from_secs(5);
                while at(
                    &api::snapshot(&core, device),
                    step["path"].as_str().unwrap(),
                ) != &step["equals"]
                {
                    assert!(Instant::now() < deadline, "{label}: timed out");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
            "execute" => {
                let result = api::execute(
                    &core,
                    device,
                    step["command"].as_str().unwrap(),
                    &step["params"],
                )
                .await;
                assert_eq!(strip_messages(&result), step["expect"], "{label}");
            }
            "close" => core.close(device).await,
            other => panic!("unknown op {other}"),
        }
    }
    drop(sim.stdin.take());
    let _ = sim.wait();
}
