//! An Analog Way AWJ device simulated on real TCP, driven through the public
//! API: JSON objects ended by 0x04, get answered with {"path", "value"},
//! replace unanswered, errors as {"error": {"code", "message"}}, and pushes
//! for subscribed path prefixes (Aquilon AWJ guide v6.2 and Midra 4K AWJ
//! guide v3.2, sections 1.2-1.4).
//!
//! The simulator holds a flat map of properties. A get of a known property
//! answers its value; a get of the transition object answers it nested, as
//! an object; anything else answers E12. A TAKE moves the T-bar and pushes
//! the new position, as the device does for a subscribed client.
//!
//! The TPP tests at the end drive the spec-driven LiveCore and Midra specs
//! against a TPP device that answers each command line with its register,
//! checking the full register sequences the vectors can only begin.

#![cfg(feature = "analogway")]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Log = Arc<Mutex<Vec<Value>>>;

fn framed(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(v).unwrap();
    b.push(0x04);
    b
}

struct Device {
    props: BTreeMap<String, Value>,
    subscriptions: Vec<String>,
    /// xTake path to the transition path it moves.
    takes: BTreeMap<String, String>,
    /// An object path answered as the nested form of the properties under it.
    object: String,
}

impl Device {
    fn pushed(&self, path: &str, value: &Value) -> Option<Value> {
        self.subscriptions
            .iter()
            .any(|s| path.starts_with(s.as_str()))
            .then(|| json!({"path": path, "value": value}))
    }

    fn answer(&mut self, req: &Value) -> Vec<Value> {
        let path = req["path"].as_str().unwrap().to_string();
        match req["op"].as_str() {
            Some("get") => {
                if let Some(v) = self.props.get(&path) {
                    return vec![json!({"path": path, "value": v})];
                }
                if path == self.object {
                    let mut nested = json!({});
                    for (p, v) in self.props.range(path.clone()..) {
                        let Some(rest) = p.strip_prefix(&format!("{path}/")) else {
                            break;
                        };
                        let mut node = &mut nested;
                        let parts: Vec<&str> = rest.split('/').collect();
                        for part in &parts[..parts.len() - 1] {
                            node = node
                                .as_object_mut()
                                .unwrap()
                                .entry(*part)
                                .or_insert(json!({}));
                        }
                        node[parts[parts.len() - 1]] = v.clone();
                    }
                    return vec![json!({"path": path, "value": nested})];
                }
                vec![
                    json!({"error": {"code": "E12", "message": format!("Unexpected path \"{path}\"")}}),
                ]
            }
            Some("replace") => {
                let value = req["value"].clone();
                if path == "Subscriptions" {
                    self.subscriptions = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect();
                    return vec![json!({"path": "Subscriptions", "value": value})];
                }
                if let Some(t) = self.takes.get(&path).cloned() {
                    let next = if self.props[&t] == "AT_DOWN" {
                        "AT_UP"
                    } else {
                        "AT_DOWN"
                    };
                    self.props.insert(t.clone(), json!(next));
                    return self.pushed(&t, &json!(next)).into_iter().collect();
                }
                self.props.insert(path.clone(), value.clone());
                self.pushed(&path, &value).into_iter().collect()
            }
            _ => vec![json!({"error": {"code": "E11", "message": "Unexpected operator"}})],
        }
    }
}

async fn simulate(device: Device) -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut device = device;
        let mut buf = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            let mut reply = Vec::new();
            while let Some(end) = pending.iter().position(|&b| b == 0x04) {
                let msg: Vec<u8> = pending.drain(..=end).collect();
                let req: Value = serde_json::from_slice(&msg[..msg.len() - 1]).unwrap();
                record.lock().unwrap().push(req.clone());
                for out in device.answer(&req) {
                    reply.extend(framed(&out));
                }
            }
            if !reply.is_empty() {
                stream.write_all(&reply).await.unwrap();
            }
        }
    });
    (port, log)
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

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn props(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

const LP_T1: &str = "DeviceObject/$screenAuxGroup/@items/S1/status/@props/transition";

#[tokio::test(flavor = "multi_thread")]
async fn livepremier_end_to_end() {
    let device = Device {
        props: props(&[
            (
                "DeviceObject/system/$device/@items/1/@props/dev",
                json!("NLC_RS4"),
            ),
            (
                "DeviceObject/system/$device/@items/1/serial/@props/serialNumber",
                json!("XX9999"),
            ),
            (
                "DeviceObject/system/$device/@items/1/version/@props/updater",
                json!("2.2.80"),
            ),
            (LP_T1, json!("AT_DOWN")),
            (
                "DeviceObject/$screen/@items/S1/control/@props/label",
                json!("Sc1"),
            ),
        ]),
        subscriptions: Vec::new(),
        takes: [(
            "DeviceObject/$screenAuxGroup/@items/S1/control/@props/xTake".to_string(),
            LP_T1.to_string(),
        )]
        .into(),
        object: "DeviceObject/$screenAuxGroup".into(),
    };
    let (port, log) = simulate(device).await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "analogway-livepremier".into(),
            model: "aquilon-rs4".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"extra_subscriptions": "DeviceObject/audio"})),
        })
        .unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;

    // The device type, serial and firmware, and the T-bar position from the
    // transition object read on connecting.
    wait_for_state(&core, id, |s| {
        s["device"]["type"] == "NLC_RS4"
            && s["device"]["firmware"] == "2.2.80"
            && s["screens"]["1"]["transition"] == "AT_DOWN"
    })
    .await;

    // Program at AT_DOWN is bank A; the write is pushed back and becomes
    // the screen's program state.
    assert_eq!(
        core.execute(
            id,
            "set_layer_source",
            params(json!({"screen": 1, "layer": 2, "source": "LIVE_3", "destination": "program"}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| {
        s["screens"]["1"]["program"]["layers"]["2"]["source"] == "LIVE_3"
            && s["screens"]["1"]["banks"]["A"]["layers"]["2"]["source"] == "LIVE_3"
    })
    .await;

    // A TAKE moves the T-bar: what was on program is now on preview.
    assert_eq!(
        core.execute(id, "take", params(json!({"screen": 1}))).await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| {
        s["screens"]["1"]["transition"] == "AT_UP"
            && s["screens"]["1"]["preview"]["layers"]["2"]["source"] == "LIVE_3"
            && s["screens"]["1"]["program"]["layers"]["2"].is_null()
    })
    .await;

    // Program is now bank B.
    assert_eq!(
        core.execute(
            id,
            "set_layer_source",
            params(json!({"screen": 1, "layer": 2, "source": "LIVE_5", "destination": "program"}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| {
        s["screens"]["1"]["program"]["layers"]["2"]["source"] == "LIVE_5"
    })
    .await;

    // Reads.
    assert_eq!(
        core.execute(
            id,
            "awj_get",
            params(json!({"path": "DeviceObject/$screen/@items/S1/control/@props/label"}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!("Sc1")
        })
    );
    assert!(matches!(
        core.execute(id, "awj_get", params(json!({"path": "DeviceObject/system/@props/div"}))).await,
        Err(CommandError::DeviceError { code: Some(c), .. }) if c == "E12"
    ));

    let sent = log.lock().unwrap().clone();
    assert_eq!(
        sent[0],
        json!({"op": "get", "path": "DeviceObject/system/$device/@items/1/@props/dev"})
    );
    assert_eq!(sent[1]["path"], "Subscriptions");
    let subs = sent[1]["value"].as_array().unwrap();
    assert!(subs.contains(&json!("DeviceObject/$screenAuxGroup")));
    assert!(subs.contains(&json!("DeviceObject/audio")));
    let writes: Vec<&str> = sent
        .iter()
        .filter(|r| r["op"] == "replace")
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        &writes[1..],
        [
            "DeviceObject/$screen/@items/S1/$preset/@items/A/$layer/@items/2/source/@props/inputNum",
            "DeviceObject/$screenAuxGroup/control/@props/xUpdate",
            "DeviceObject/$screenAuxGroup/@items/S1/control/@props/xTake",
            "DeviceObject/$screen/@items/S1/$preset/@items/B/$layer/@items/2/source/@props/inputNum",
            "DeviceObject/$screenAuxGroup/control/@props/xUpdate",
        ]
    );
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn midra_reads_the_tbar_before_a_banked_write() {
    let t = "DeviceObject/transition/$auxiliaryScreen/@items/1/status/@props/transition";
    let device = Device {
        props: props(&[
            ("DeviceObject/system/@props/dev", json!("PULSE")),
            (t, json!("AT_DOWN")),
        ]),
        subscriptions: Vec::new(),
        takes: BTreeMap::new(),
        object: "none".into(),
    };
    let (port, log) = simulate(device).await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "analogway-midra4k".into(),
            model: "pulse-4k".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"read_on_connect": false})),
        })
        .unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    wait_for_state(&core, id, |s| s["device"]["type"] == "PULSE").await;

    assert_eq!(
        core.execute(
            id,
            "set_aux_source",
            params(json!({"aux": 1, "source": "INPUT_2", "destination": "program"}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| {
        s["auxiliaries"]["1"]["program"]["source"] == "INPUT_2"
    })
    .await;

    let sent = log.lock().unwrap().clone();
    let at = sent
        .iter()
        .position(|r| r == &json!({"op": "get", "path": t}))
        .expect("the T-bar is read first");
    assert_eq!(
        sent[at + 1],
        json!({"op": "replace",
            "path": "DeviceObject/$auxiliaryScreen/@items/1/$preset/@items/DOWN/background/source/@props/content",
            "value": "INPUT_2"})
    );
    core.close(id).await;
}

// ── TPP (LiveCore and Midra), spec-driven ─────────────────────────────────

type Lines = Arc<Mutex<Vec<String>>>;

/// A TPP device: every LF-ended command is answered with its register name
/// first, then its indexes and value, and CR LF (LiveCore TPP guide 2.4.4),
/// "?" with DEV<type> and "*" with *1 (guide 3.1).
async fn simulate_tpp(device_type: &'static str) -> (u16, Lines) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Lines = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            let mut reply = String::new();
            while let Some(end) = pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = pending.drain(..=end).collect();
                let line = String::from_utf8(line[..line.len() - 1].to_vec()).unwrap();
                record.lock().unwrap().push(line.clone());
                let answer = match line.as_str() {
                    "?" => format!("DEV{device_type}"),
                    "*" => "*1".to_string(),
                    _ => {
                        let at = line
                            .rfind(|c: char| !c.is_ascii_alphabetic() && c != '#')
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        let (args, register) = line.split_at(at);
                        format!("{register}{}", args.trim_end_matches(','))
                    }
                };
                reply.push_str(&answer);
                reply.push_str("\r\n");
            }
            if !reply.is_empty() {
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        }
    });
    (port, log)
}

#[tokio::test(flavor = "multi_thread")]
async fn livecore_recall_sends_every_register_in_order() {
    let (port, log) = simulate_tpp("113").await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "analogway-livecore".into(),
            model: "ascender-16-4k".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
        })
        .unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    wait_for_state(&core, id, |s| s["device"]["type"] == 113).await;

    // Guide 3.2: filter, scale, memory - 1, screen - 1, destination, load.
    assert_eq!(
        core.execute(
            id,
            "recall_preset",
            params(json!({"memory": 4, "screen": 2}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    // Guide 3.4: the answer is also state.
    assert_eq!(
        core.execute(
            id,
            "set_layer_source",
            params(json!({"screen": 1, "layer": 2, "source": 4}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["screens"]["1"]["preview"]["layers"]["2"]["source"] == 4
    })
    .await;

    let lines = log.lock().unwrap().clone();
    let at = lines
        .iter()
        .position(|l| l == "4095PMcat")
        .expect("the recall");
    assert_eq!(
        &lines[at..at + 6],
        [
            "4095PMcat",
            "0PMlse",
            "3PMmet",
            "1PMscf",
            "1PMprf",
            "1PMloa"
        ]
    );
    assert!(lines[at + 6..].iter().any(|l| l == "0,1,1,4SPPEi"));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn midra_layer_change_is_followed_by_the_screen_update() {
    let (port, log) = simulate_tpp("259").await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "analogway-midra".into(),
            model: "pulse2".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({})),
        })
        .unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    wait_for_state(&core, id, |s| s["device"]["ready"] == true).await;

    // Guide 3.3: PRinp, then PUscu for the screen.
    assert_eq!(
        core.execute(
            id,
            "set_layer_source",
            params(json!({"screen": 1, "layer": 1, "source": 3}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["screens"]["1"]["preview"]["layers"]["1"]["source"] == 3
    })
    .await;
    let lines = log.lock().unwrap().clone();
    let at = lines
        .iter()
        .position(|l| l == "0,1,1,3PRinp")
        .expect("the layer change");
    assert_eq!(lines[at + 1], "0,1PUscu");
    core.close(id).await;
}
