//! Plays tests/bindings/script.json through the C interface against
//! integrations-sim: every call crosses the C ABI as NUL-terminated strings,
//! exactly as a C or C++ host makes it.

use std::ffi::{c_char, CStr, CString};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use meros_integrations_c::*;
use serde_json::{Map, Value};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Take ownership of a returned string, parse it, and free it through the API.
fn take(s: *mut c_char) -> Value {
    assert!(!s.is_null());
    let v = serde_json::from_str(unsafe { CStr::from_ptr(s) }.to_str().unwrap()).unwrap();
    unsafe { mi_string_free(s) };
    v
}

fn cs(v: &str) -> CString {
    CString::new(v).unwrap()
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

#[test]
fn the_shared_binding_script() {
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
    let mut vars = serde_json::from_str::<Value>(&line)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();

    let script: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/bindings/script.json")).unwrap(),
    )
    .unwrap();
    let core = mi_core_new();
    assert!(!core.is_null());

    for (index, raw) in script["steps"].as_array().unwrap().iter().enumerate() {
        let step = substitute(raw, &vars);
        let label = format!("step {index}: {raw}");
        let device = step.get("device").and_then(Value::as_u64).unwrap_or(0);
        unsafe {
            match step["op"].as_str().unwrap() {
                "catalog" => {
                    let catalog = take(mi_catalog(core));
                    assert_eq!(
                        at(&catalog, step["path"].as_str().unwrap()),
                        &step["expect"],
                        "{label}"
                    );
                }
                "open" => {
                    let request = cs(&step["request"].to_string());
                    let result = take(mi_open(core, request.as_ptr()));
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
                        &take(mi_snapshot(core, device)),
                        step["path"].as_str().unwrap(),
                    ) != &step["equals"]
                    {
                        assert!(Instant::now() < deadline, "{label}: timed out");
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                "execute" => {
                    let command = cs(step["command"].as_str().unwrap());
                    let params = cs(&step["params"].to_string());
                    let result = take(mi_execute(core, device, command.as_ptr(), params.as_ptr()));
                    assert_eq!(strip_messages(&result), step["expect"], "{label}");
                }
                "close" => mi_close(core, device),
                other => panic!("unknown op {other}"),
            }
        }
    }

    unsafe {
        // Malformed JSON is an error result, not a crash.
        let bad = cs("{not json");
        let result = take(mi_open(core, bad.as_ptr()));
        assert_eq!(result["error"]["error"], "invalid_request");
        // Events queued throughout; a bounded wait hands them over.
        let events = take(mi_wait_events(core, 1000, 100));
        assert!(events
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["event"] == "connection"));
        mi_core_free(core);
    }
    drop(sim.stdin.take());
    let _ = sim.wait();
}
