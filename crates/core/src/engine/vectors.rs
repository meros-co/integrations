//! Runs every conformance vector in `vectors/` against the spec engine.
//!
//! A vector states, independently of this code, the exact bytes a command must
//! put on the wire and, where given, what a device reply must produce. Any
//! change to rendering, framing or encoding that alters the wire fails here.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use super::SpecEngine;
use crate::catalog::{validate, Catalog};
use crate::module::{Action, Cx, HttpResponse, Module, OpenContext, TcpInput};

const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10));

fn vector_files() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors");
    let mut files = Vec::new();
    for dir in std::fs::read_dir(&root).unwrap().flatten() {
        if dir.path().is_dir() {
            for f in std::fs::read_dir(dir.path()).unwrap().flatten() {
                if f.path().extension().is_some_and(|e| e == "yaml") {
                    files.push(f.path());
                }
            }
        }
    }
    files.sort();
    files
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// What the engine put on the wire, in the form vectors state it.
#[derive(Debug, PartialEq)]
enum Wire {
    Bytes(Vec<u8>),
    Http {
        method: String,
        target: String,
        body: Option<String>,
    },
}

fn wire(actions: &[Action]) -> Vec<Wire> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::TcpSend { data, .. } | Action::UdpSend { data, .. } => {
                Some(Wire::Bytes(data.clone()))
            }
            Action::Http { request, .. } => {
                let after_host = request.url.splitn(4, '/').nth(3).unwrap_or("");
                Some(Wire::Http {
                    method: request.method.to_string(),
                    target: format!("/{after_host}"),
                    body: request
                        .body
                        .as_ref()
                        .map(|b| String::from_utf8_lossy(b).into_owned()),
                })
            }
            _ => None,
        })
        .collect()
}

fn http_ids(actions: &[Action]) -> Vec<u64> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Http { id, .. } => Some(*id),
            _ => None,
        })
        .collect()
}

fn expected_wire(v: &Value) -> Option<Vec<Wire>> {
    let list = |x: &Value| -> Vec<Value> {
        match x {
            Value::Array(a) => a.clone(),
            one => vec![one.clone()],
        }
    };
    if let Some(w) = v.get("expect_wire") {
        return Some(
            list(w)
                .iter()
                .map(|s| Wire::Bytes(s.as_str().unwrap().as_bytes().to_vec()))
                .collect(),
        );
    }
    if let Some(w) = v.get("expect_wire_hex") {
        return Some(
            list(w)
                .iter()
                .map(|s| Wire::Bytes(unhex(s.as_str().unwrap())))
                .collect(),
        );
    }
    if let Some(r) = v.get("expect_request") {
        return Some(
            list(r)
                .iter()
                .map(|r| Wire::Http {
                    method: r["method"].as_str().unwrap().to_string(),
                    target: r["target"].as_str().unwrap().to_string(),
                    body: r.get("body").and_then(Value::as_str).map(String::from),
                })
                .collect(),
        );
    }
    None
}

/// A telemetry vector: what the engine sends on connecting, and the state an
/// inbound message produces.
fn run_telemetry(v: &Value, catalog: &Catalog) -> Result<(), String> {
    let spec_id = v["spec"].as_str().ok_or("no spec")?;
    let spec = catalog
        .device(spec_id)
        .ok_or(format!("unknown spec {spec_id}"))?;
    let model = &spec.models[0];
    let settings =
        validate(&spec.settings, &Default::default()).map_err(|e| format!("settings: {e}"))?;
    let ctx = OpenContext {
        host: HOST,
        port: None,
        model: model.id.clone(),
        channels: model.channels,
        settings,
    };
    let mut engine = SpecEngine::new(Arc::new(spec.clone()), ctx)?;
    let mut cx = Cx::new(0);
    engine.start(&mut cx);
    let mut connected = cx.take();
    if connected
        .iter()
        .any(|a| matches!(a, Action::TcpOpen { .. }))
    {
        let mut cx = Cx::new(1);
        engine.tcp(&mut cx, "device", TcpInput::Connected);
        connected = cx.take();
    }
    let expect_connect = if let Some(w) = v.get("expect_connect_wire") {
        Some(expected_wire(&json!({ "expect_wire": w })).unwrap())
    } else {
        v.get("expect_connect_wire_hex")
            .map(|w| expected_wire(&json!({ "expect_wire_hex": w })).unwrap())
    };
    if let Some(expected) = expect_connect {
        let sent = wire(&connected);
        if sent != expected {
            return Err(format!(
                "connect wire mismatch\n  expected {expected:?}\n  sent     {sent:?}"
            ));
        }
    }

    let mut cx = Cx::new(3);
    if let Some(text) = v.get("inbound").and_then(Value::as_str) {
        engine.tcp(&mut cx, "device", TcpInput::Data(text.as_bytes().to_vec()));
    } else if let Some(h) = v.get("inbound_hex").and_then(Value::as_str) {
        engine.datagram(&mut cx, "device", SocketAddr::new(HOST, 1), &unhex(h));
    } else if let Some(r) = v.get("inbound_http") {
        // The reply to a request for this path, as the engine offers it.
        let path = r["path"].as_str().ok_or("inbound_http needs a path")?;
        let body = r["body"].as_str().unwrap_or("").as_bytes();
        if let Some(patch) = engine
            .telemetry
            .apply(&super::telemetry::Inbound::Http { path, body })
        {
            cx.state(patch);
        }
        // As the engine does: a text reply also goes to the text rules.
        if let Ok(text) = std::str::from_utf8(body) {
            engine.apply_text(&mut cx, text);
        }
    }
    let mut state = json!({});
    for a in cx.take() {
        if let Action::State(p) = a {
            crate::session::merge_patch(&mut state, &p);
        }
    }
    let expected = v
        .get("expect_state")
        .ok_or("vector states no expect_state")?;
    if &state != expected {
        return Err(format!(
            "state mismatch\n  expected {expected}\n  got      {state}"
        ));
    }
    Ok(())
}

fn run(path: &PathBuf, catalog: &Catalog) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap();
    let v: Value = serde_yaml::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    if ["inbound", "inbound_hex", "inbound_http"]
        .iter()
        .any(|k| v.get(k).is_some())
    {
        return run_telemetry(&v, catalog);
    }
    let spec_id = v["spec"].as_str().ok_or("no spec")?;
    let command = v["command"].as_str().ok_or("no command")?;
    let spec = catalog
        .device(spec_id)
        .ok_or(format!("unknown spec {spec_id}"))?;
    let command_spec = spec
        .commands
        .get(command)
        .ok_or(format!("unknown command {command}"))?;
    let model = match v.get("model").and_then(Value::as_str) {
        Some(m) => spec.model(m).ok_or(format!("unknown model {m}"))?,
        None => spec
            .models
            .iter()
            .find(|m| m.supports.iter().any(|c| c == command))
            .ok_or("no model supports the command")?,
    };
    if !model.supports.iter().any(|c| c == command) {
        return Err(format!("model {} does not support {command}", model.id));
    }

    let input = v
        .get("input")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let settings = v
        .get("settings")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let params = validate(&command_spec.params, &input).map_err(|e| format!("input: {e}"))?;
    let settings = validate(&spec.settings, &settings).map_err(|e| format!("settings: {e}"))?;

    let ctx = OpenContext {
        host: HOST,
        port: None,
        model: model.id.clone(),
        channels: model.channels,
        settings,
    };
    let mut engine = SpecEngine::new(Arc::new(spec.clone()), ctx)?;

    // Bring the session up: connect, and answer any probe the engine sends
    // first, so only the command's own messages are compared.
    let mut cx = Cx::new(0);
    engine.start(&mut cx);
    let started = cx.take();
    let mut cx = Cx::new(1);
    if started.iter().any(|a| matches!(a, Action::TcpOpen { .. })) {
        engine.tcp(&mut cx, "device", TcpInput::Connected);
    }
    for id in http_ids(&started) {
        engine.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                status: 200,
                body: b"{}".to_vec(),
            }),
        );
    }
    if started.iter().any(|a| matches!(a, Action::UdpSend { .. })) && engine.current.is_some() {
        let probe_reply = super::osc::encode("/probe-reply", &[]);
        engine.datagram(&mut cx, "device", SocketAddr::new(HOST, 1), &probe_reply);
    }
    // Telemetry subscriptions and queries are queued ahead of commands.
    // Complete them with a plain success reply, so the vector sees only its
    // command: "200 ok" (HyperDeck), or a Protocol 3000 device line where the
    // transport takes only lines matching its reply_match as replies. OSC
    // telemetry queries are answered on their own address.
    let success: &[u8] = match &engine.transport {
        super::Transport::LineTcp {
            reply_match: Some(re),
            ..
        } if !re.is_match("200 ok") => b"~01@ok\r\n",
        _ => b"200 ok\r\n",
    };
    let mut guard = 0;
    while engine.current.as_ref().is_some_and(|f| f.id.is_none()) && guard < 65536 {
        let awaiting = engine.current.as_ref().and_then(|f| f.awaiting.as_ref());
        match awaiting {
            Some(super::Await::Osc(address)) => {
                let reply = super::osc::encode(address.as_deref().unwrap_or("/probe-reply"), &[]);
                engine.datagram(&mut cx, "device", SocketAddr::new(HOST, 1), &reply);
            }
            _ => engine.tcp(&mut cx, "device", TcpInput::Data(success.to_vec())),
        }
        guard += 1;
    }
    if engine.current.as_ref().is_some_and(|f| f.id.is_none()) {
        return Err("the connection sequence did not complete".into());
    }
    cx.take();

    let mut cx = Cx::new(2);
    engine.command(&mut cx, 1, command, &params);
    let actions = cx.take();
    let sent = wire(&actions);

    if let Some(expected) = expected_wire(&v) {
        if sent != expected {
            let show = |w: &[Wire]| -> Vec<String> {
                w.iter()
                    .map(|x| match x {
                        Wire::Bytes(b) => format!("{:?} ({})", String::from_utf8_lossy(b), hex(b)),
                        other => format!("{other:?}"),
                    })
                    .collect()
            };
            return Err(format!(
                "wire mismatch\n  expected {:?}\n  sent     {:?}",
                show(&expected),
                show(&sent)
            ));
        }
    } else {
        return Err("vector states no expected wire".into());
    }

    let mut done: Vec<Value> = actions
        .iter()
        .filter_map(|a| match a {
            Action::Complete { result, .. } => Some(crate::json::result(result)),
            _ => None,
        })
        .collect();

    if let Some(reply) = v.get("device_reply").and_then(Value::as_str) {
        let mut cx = Cx::new(3);
        engine.tcp(&mut cx, "device", TcpInput::Data(reply.as_bytes().to_vec()));
        done.extend(cx.take().iter().filter_map(|a| match a {
            Action::Complete { result, .. } => Some(crate::json::result(result)),
            _ => None,
        }));
    }
    if let Some(reply) = v.get("device_reply_hex").and_then(Value::as_str) {
        let mut cx = Cx::new(3);
        engine.datagram(&mut cx, "device", SocketAddr::new(HOST, 1), &unhex(reply));
        done.extend(cx.take().iter().filter_map(|a| match a {
            Action::Complete { result, .. } => Some(crate::json::result(result)),
            _ => None,
        }));
    }
    if let Some(reply) = v.get("http_reply") {
        let mut cx = Cx::new(3);
        let status = reply["status"].as_u64().unwrap_or(200) as u16;
        let body = reply.get("body").map(|b| match b {
            Value::String(s) => s.clone().into_bytes(),
            other => other.to_string().into_bytes(),
        });
        let id = http_ids(&actions)[0];
        engine.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                status,
                body: body.unwrap_or_default(),
            }),
        );
        done.extend(cx.take().iter().filter_map(|a| match a {
            Action::Complete { result, .. } => Some(crate::json::result(result)),
            _ => None,
        }));
    }

    if let Some(expected) = v.get("expect_result") {
        // Compare without the human-readable message, which is not part of
        // the contract.
        let strip = |mut x: Value| {
            if let Some(e) = x.get_mut("error").and_then(Value::as_object_mut) {
                e.remove("message");
            }
            x
        };
        let got = done.first().cloned().map(strip).unwrap_or(json!(null));
        if got != strip(expected.clone()) {
            return Err(format!(
                "result mismatch\n  expected {expected}\n  got      {got}"
            ));
        }
    }
    Ok(())
}

#[test]
fn every_vector_passes() {
    let catalog = Catalog::embedded();
    let files = vector_files();
    assert!(!files.is_empty(), "no vectors found");
    let failures: Vec<String> = files
        .iter()
        .filter_map(|f| {
            run(f, &catalog)
                .err()
                .map(|e| format!("{}: {e}", f.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} vectors failed:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

#[test]
fn every_spec_driven_command_has_a_vector() {
    let catalog = Catalog::embedded();
    let mut covered = std::collections::BTreeSet::new();
    for f in vector_files() {
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let Some(command) = v["command"].as_str() else {
            continue;
        };
        covered.insert(format!("{}/{command}", v["spec"].as_str().unwrap()));
    }
    let missing: Vec<String> = catalog
        .devices
        .values()
        .filter(|s| s.implementation == crate::catalog::Implementation::Spec)
        .flat_map(|s| s.commands.keys().map(move |c| format!("{}/{c}", s.id)))
        .filter(|k| !covered.contains(k))
        .collect();
    assert!(
        missing.is_empty(),
        "commands without a vector:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_spec_with_telemetry_has_a_telemetry_vector_and_constructs() {
    let catalog = Catalog::embedded();
    let mut covered = std::collections::BTreeSet::new();
    for f in vector_files() {
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        if ["inbound", "inbound_hex", "inbound_http"]
            .iter()
            .any(|k| v.get(k).is_some())
        {
            covered.insert(v["spec"].as_str().unwrap().to_string());
        }
    }
    for spec in catalog.devices.values() {
        if spec.implementation != crate::catalog::Implementation::Spec {
            continue;
        }
        let model = &spec.models[0];
        let ctx = OpenContext {
            host: HOST,
            port: None,
            model: model.id.clone(),
            channels: model.channels,
            settings: validate(&spec.settings, &Default::default()).unwrap_or_default(),
        };
        if let Err(e) = SpecEngine::new(Arc::new(spec.clone()), ctx) {
            panic!("{}: {e}", spec.id);
        }
        if spec.telemetry.is_some() {
            assert!(
                covered.contains(&spec.id),
                "{} has telemetry but no telemetry vector",
                spec.id
            );
        }
    }
}
