//! Plays tests/bindings/script.json through the C interface against
//! integrations-sim: every call crosses the C ABI as NUL-terminated strings,
//! exactly as a C or C++ host makes it.

// The script drives devices from several families.
#![cfg(feature = "all")]

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
                "stream" => {
                    let name = cs(step["stream"].as_str().unwrap());
                    let mut stream: *mut MiStream = std::ptr::null_mut();
                    let result = take(mi_stream_open(core, device, name.as_ptr(), &mut stream));
                    if let Some(expect) = step.get("expect") {
                        assert_eq!(&strip_messages(&result), expect, "{label}");
                        assert!(stream.is_null(), "{label}");
                        continue;
                    }
                    assert_eq!(result, serde_json::json!({"ok": true}), "{label}");
                    assert!(!stream.is_null());
                    let mut last = 0;
                    for _ in 0..step["frames"].as_u64().unwrap() {
                        let frame = mi_stream_wait(core, stream, 5000);
                        assert!(!frame.is_null(), "{label}: no frame");
                        let f = &*frame;
                        let data = std::slice::from_raw_parts(f.data, f.len);
                        assert!(data.starts_with(&[0xFF, 0xD8]), "{label}");
                        assert_eq!(
                            CStr::from_ptr(f.format).to_str().unwrap(),
                            step["format"],
                            "{label}"
                        );
                        assert!(f.sequence > last, "{label}");
                        last = f.sequence;
                        mi_frame_free(frame);
                    }
                    assert_eq!(mi_stream_ended(stream), 0);
                    mi_stream_close(stream);
                    assert!(mi_stream_wait(core, stream, 1000).is_null());
                    assert_eq!(mi_stream_ended(stream), 1);
                    mi_stream_free(stream);
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

/// A core started for some devices, through the C constructors.
#[test]
fn a_core_started_for_some_devices() {
    unsafe {
        let options = cs(r#"{"devices":["sennheiser-ew-g3-g4","shure-wireless"]}"#);
        let core = mi_core_create(options.as_ptr(), std::ptr::null_mut());
        assert!(!core.is_null());
        let catalog = take(mi_catalog(core));
        let ids: Vec<&String> = catalog["devices"].as_object().unwrap().keys().collect();
        assert_eq!(ids, ["sennheiser-ew-g3-g4", "shure-wireless"]);

        let request = cs(r#"{"device":"kramer-p3000","model":"p3000-generic","host":"127.0.0.1"}"#);
        let refused = take(mi_open(core, request.as_ptr()));
        assert_eq!(refused["error"]["error"], "not_selected");

        let request = cs(r#"{"action":"scan","protocols":["ssdp"]}"#);
        let refused = take(mi_discover(core, request.as_ptr()));
        assert_eq!(refused["error"]["error"], "invalid_request");
        mi_core_free(core);

        let options = cs(r#"{"devices":["no-such-device"]}"#);
        assert!(mi_core_new_with_options(options.as_ptr()).is_null());
        let mut error: *mut c_char = std::ptr::null_mut();
        assert!(mi_core_create(options.as_ptr(), &mut error).is_null());
        let error = take(error);
        assert_eq!(error["error"]["error"], "invalid_options");
        assert!(error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no-such-device"));
    }
}
