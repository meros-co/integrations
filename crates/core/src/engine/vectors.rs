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
use crate::module::{Action, Cx, HttpResponse, Module, OpenContext, SseInput, TcpInput, WsInput};

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
#[derive(Debug)]
enum Wire {
    Bytes(Vec<u8>),
    Http {
        method: String,
        /// The port the request went to: the transport's own, or an
        /// endpoint's (`transport.endpoints`).
        port: u16,
        target: String,
        /// The body as text, or `hex:` and its bytes when it is not UTF-8
        /// (a binary body, `body_hex` in a vector).
        body: Option<String>,
        /// The request's Content-Type; compared only when the vector gives
        /// one.
        content_type: Option<String>,
    },
}

/// `sent == expected`: an expected request without a content type matches
/// any.
impl PartialEq for Wire {
    fn eq(&self, expected: &Wire) -> bool {
        match (self, expected) {
            (Wire::Bytes(a), Wire::Bytes(b)) => a == b,
            (
                Wire::Http {
                    method,
                    port,
                    target,
                    body,
                    content_type,
                },
                Wire::Http {
                    method: m,
                    port: p,
                    target: t,
                    body: b,
                    content_type: c,
                },
            ) => {
                method == m
                    && port == p
                    && target == t
                    && body == b
                    && (c.is_none() || content_type == c)
            }
            _ => false,
        }
    }
}

/// A body as vectors state it: its text, or `hex:` and its bytes.
fn body_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => format!(
            "hex:{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ),
    }
}

/// The port a URL names.
fn url_port(url: &str) -> u16 {
    let authority = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or("");
    authority
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse().ok())
        .unwrap_or(0)
}

/// The transport's own HTTP port, which a vector's request goes to unless it
/// names another.
fn main_port(engine: &SpecEngine) -> u16 {
    match &engine.transport {
        super::Transport::Http { base, .. } => url_port(base),
        _ => 0,
    }
}

fn wire(actions: &[Action]) -> Vec<Wire> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::TcpSend { data, .. } | Action::UdpSend { data, .. } => {
                Some(Wire::Bytes(data.clone()))
            }
            Action::WsSend {
                socket: "device",
                text,
            } => Some(Wire::Bytes(text.clone().into_bytes())),
            Action::Http { request, .. } => {
                let after_host = request.url.splitn(4, '/').nth(3).unwrap_or("");
                Some(Wire::Http {
                    method: request.method.to_string(),
                    port: url_port(&request.url),
                    target: format!("/{after_host}"),
                    // A body streamed from host files, read whole to compare.
                    body: request
                        .body_bytes()
                        .unwrap_or_else(|e| panic!("reading the body's files: {e}"))
                        .as_deref()
                        .map(body_text),
                    content_type: request
                        .headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                        .map(|(_, v)| v.clone()),
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

/// The wire a vector states; an HTTP request without `port` goes to
/// `main_port`.
fn expected_wire(v: &Value, main_port: u16) -> Option<Vec<Wire>> {
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
                    port: r
                        .get("port")
                        .and_then(Value::as_u64)
                        .map_or(main_port, |p| p as u16),
                    target: r["target"].as_str().unwrap().to_string(),
                    body: match r.get("body_hex").and_then(Value::as_str) {
                        Some(hex) => Some(body_text(&unhex(hex))),
                        None => r.get("body").and_then(Value::as_str).map(String::from),
                    },
                    content_type: r
                        .get("content_type")
                        .and_then(Value::as_str)
                        .map(String::from),
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
    // The model the device is opened as: the first, or the one the vector
    // names for rules that apply to some models only (SPEC.md §8).
    let model = match v.get("model").and_then(Value::as_str) {
        Some(m) => spec.model(m).ok_or(format!("unknown model {m}"))?,
        None => &spec.models[0],
    };
    // Optional device settings, as for a command vector: a spec whose
    // telemetry names a required setting (the plan to follow) needs them.
    let settings = v
        .get("settings")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    // A serial port has no default; a vector need not name one.
    let mut settings = settings;
    for (name, value) in serial_port(spec) {
        settings.entry(name).or_insert(value);
    }
    let settings = crate::catalog::validate_settings(&spec.settings, &settings, &model.id)
        .map_err(|e| format!("settings: {e}"))?;
    let ctx = OpenContext {
        host: HOST,
        host_name: None,
        port: vector_port(spec),
        model: model.id.clone(),
        channels: model.channels,
        settings,
        monitor: true,
    };
    let link = spec.serial_link(&model.id, &ctx.settings);
    let mut engine = SpecEngine::new(Arc::new(spec.clone()), ctx)?;
    // Vectors check what is sent, not when: pacing (SPEC.md §2) has its own
    // engine tests.
    engine.pacer = super::pacing::Pacer::default();
    let mut cx = Cx::new(0);
    engine.start(&mut cx);
    let mut connected = cx.take();
    check_link(link, &connected)?;
    if connected.iter().any(|a| {
        matches!(
            a,
            Action::TcpOpen { .. } | Action::TcpOpenTls { .. } | Action::SerialOpen { .. }
        )
    }) {
        let mut cx = Cx::new(1);
        engine.tcp(&mut cx, "device", TcpInput::Connected);
        // A login that waits for the device's prompts is given each in turn
        // (Sharp's user name, then password).
        for _ in 0..8 {
            let Some((_, prompt, _)) = engine.prompt_wait.clone() else {
                break;
            };
            engine.tcp(&mut cx, "device", TcpInput::Data(prompt.into_bytes()));
        }
        connected = cx.take();
    }
    if connected.iter().any(|a| {
        matches!(
            a,
            Action::WsOpen {
                socket: "device",
                ..
            }
        )
    }) {
        let mut cx = Cx::new(1);
        engine.ws(&mut cx, "device", WsInput::Opened);
        connected = cx.take();
    }
    // A push websocket beside the transport: what is sent on opening it.
    if engine.push.is_some() {
        let mut cx = Cx::new(2);
        engine.ws(&mut cx, "push", WsInput::Opened);
        let sent: Vec<String> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::WsSend {
                    socket: "push",
                    text,
                } => Some(text),
                _ => None,
            })
            .collect();
        if let Some(expected) = v.get("expect_connect_ws") {
            let expected: Vec<String> = serde_json::from_value(expected.clone())
                .map_err(|e| format!("expect_connect_ws: {e}"))?;
            if sent != expected {
                return Err(format!(
                    "websocket connect mismatch
  expected {expected:?}
  sent     {sent:?}"
                ));
            }
        }
    }
    let expect_connect = if let Some(w) = v.get("expect_connect_wire") {
        Some(expected_wire(&json!({ "expect_wire": w }), 0).unwrap())
    } else {
        v.get("expect_connect_wire_hex")
            .map(|w| expected_wire(&json!({ "expect_wire_hex": w }), 0).unwrap())
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
    // What the connection queued (polls) is dropped, so what the message
    // queues (`then_send`) is all that is left.
    engine.queue.clear();
    // The engine knows the state the message arrives on, so a list read
    // again unchanged is reported as no change.
    if let (Some(mirror), Some(before)) = (engine.mirror.as_mut(), v.get("state_before")) {
        *mirror = before.clone();
    }
    if let (Some(text), Some(request)) = (
        v.get("inbound").and_then(Value::as_str),
        v.get("request").and_then(Value::as_str),
    ) {
        // A reply, with the text of the message it answers.
        engine.offer_answer(&mut cx, text.trim_end(), request);
    } else if let Some(text) = v.get("inbound").and_then(Value::as_str) {
        engine.tcp(&mut cx, "device", TcpInput::Data(text.as_bytes().to_vec()));
    } else if let Some(h) = v.get("inbound_hex").and_then(Value::as_str) {
        // Beside a stream or HTTP transport, a datagram is OSC feedback on
        // `telemetry.osc`'s port.
        let socket = match engine.transport {
            super::Transport::OscUdp { .. }
            | super::Transport::LineUdp { .. }
            | super::Transport::OscTcp { .. } => "device",
            _ => super::OSC_LISTEN,
        };
        engine.datagram(&mut cx, socket, SocketAddr::new(HOST, 1), &unhex(h));
    } else if let Some(text) = v.get("inbound_ws").and_then(Value::as_str) {
        let socket = match engine.transport {
            super::Transport::Ws { .. } => "device",
            _ => "push",
        };
        engine.ws(&mut cx, socket, WsInput::Text(text.to_string()));
    } else if let Some(e) = v.get("inbound_sse") {
        // An event on `telemetry.sse`: its name (`message` when absent) and data.
        let event = crate::sse::SseEvent {
            event: e
                .get("event")
                .and_then(Value::as_str)
                .unwrap_or("message")
                .to_string(),
            data: e["data"]
                .as_str()
                .ok_or("inbound_sse needs data")?
                .to_string(),
        };
        engine.sse(&mut cx, super::EVENTS, SseInput::Event(event));
    } else if let Some(r) = v.get("inbound_http") {
        // The reply to a request for this path, as the engine offers it.
        let path = r["path"].as_str().ok_or("inbound_http needs a path")?;
        let body = r["body"].as_str().unwrap_or("").as_bytes();
        // The JSON body of the request it answers, for `request_match`.
        let request = r.get("request");
        // Response headers, for a rule reading them.
        let headers: Vec<(String, String)> = r
            .get("headers")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.as_str().unwrap_or("").to_string()))
            .collect();
        engine.offer(
            &mut cx,
            &super::telemetry::Inbound::Http {
                path,
                headers: &headers,
                body,
                request,
            },
        );
        // As the engine does: a text reply also goes to the text rules.
        if let Ok(text) = std::str::from_utf8(body) {
            engine.apply_text(&mut cx, text);
        }
    }
    // `state_before`: the state the message arrives on, for one that removes
    // something.
    let mut state = v.get("state_before").cloned().unwrap_or(json!({}));
    let actions = cx.take();
    for a in &actions {
        if let Action::State(p) = a {
            crate::session::merge_patch(&mut state, p);
        }
    }
    // `expect_then_send`: the requests the message queued, sent at once or
    // waiting behind the one in flight; `expect_then_send_hex` for binary
    // messages (OSC), framed as sent.
    let then_send = v
        .get("expect_then_send")
        .map(|e| (e, false))
        .or_else(|| v.get("expect_then_send_hex").map(|e| (e, true)));
    if let Some((expected, hex)) = then_send {
        let mut sent = wire(&actions);
        let waiting: Vec<super::Job> = engine.queue.drain(..).collect();
        for job in waiting {
            let flight = engine.prepare(&job)?;
            for (outgoing, _, _, _) in flight.messages {
                match outgoing {
                    super::Outgoing::Http(request, _) => {
                        sent.extend(wire(&[Action::Http { id: 0, request }]))
                    }
                    super::Outgoing::Bytes(bytes) => sent.push(Wire::Bytes(bytes)),
                    super::Outgoing::Ws(text) => sent.push(Wire::Bytes(text.into_bytes())),
                }
            }
        }
        // Requests as expect_request gives them, or line messages as text.
        let key = match expected.get(0) {
            _ if hex => "expect_wire_hex",
            Some(Value::String(_)) => "expect_wire",
            _ => "expect_request",
        };
        let expected = expected_wire(&json!({ key: expected }), main_port(&engine)).unwrap();
        if sent != expected {
            return Err(format!(
                "then_send mismatch\n  expected {expected:?}\n  sent     {sent:?}"
            ));
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

/// The port a vector opens a device on: none (the spec's default), or for a
/// device with no standard port (`port: null`) one standing for the port the
/// operator gives.
fn vector_port(spec: &crate::catalog::DeviceSpec) -> Option<u16> {
    spec.transport
        .as_ref()
        .and_then(|t| t.get("port"))
        .is_some_and(Value::is_null)
        .then_some(VECTOR_PORT)
}

/// The port a vector gives a device with no standard port.
const VECTOR_PORT: u16 = 53000;

/// Answer the internal job in flight (probe, subscription or telemetry
/// query) with a plain success: on its OSC address, or as a text line.
fn answer_internal(engine: &mut SpecEngine, cx: &mut Cx, success: &[u8]) {
    let awaiting = engine.current.as_ref().and_then(|f| f.awaiting.as_ref());
    match awaiting {
        // A reply_address pattern: no reply can be made up for it, so the
        // query times out, which leaves an addressed stream open.
        Some(super::Await::Osc(Some(super::OscReply::Pattern(_)))) => {
            engine.timer(cx, super::REPLY);
        }
        Some(super::Await::Osc(address)) => {
            let address = match address {
                Some(super::OscReply::Exact(a)) => a.clone(),
                _ => "/probe-reply".to_string(),
            };
            let reply = super::osc::encode(&address, &[]);
            engine.datagram(cx, "device", SocketAddr::new(HOST, 1), &reply);
        }
        Some(super::Await::Ws(_)) => engine.ws(cx, "device", WsInput::Text("{}".into())),
        // A query to an endpoint beside a line transport (a HyperDeck's
        // configuration API): an empty JSON answer.
        Some(super::Await::Http(id)) => {
            let id = *id;
            engine.http_response(
                cx,
                id,
                Ok(HttpResponse {
                    headers: Vec::new(),
                    status: 200,
                    body: b"{}".to_vec(),
                }),
            );
        }
        // One reply line per datagram.
        _ if matches!(engine.transport, super::Transport::LineUdp { .. }) => {
            let line = String::from_utf8_lossy(success);
            let line = line.trim_end().as_bytes().to_vec();
            engine.datagram(cx, "device", SocketAddr::new(HOST, 1), &line);
        }
        _ => engine.tcp(cx, "device", TcpInput::Data(success.to_vec())),
    }
}

fn run(path: &PathBuf, catalog: &Catalog) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap();
    let v: Value = crate::yaml::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    if [
        "inbound",
        "inbound_hex",
        "inbound_http",
        "inbound_ws",
        "inbound_sse",
    ]
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
    // A serial port has no default; a vector need not name one.
    let mut settings = settings;
    for (name, value) in serial_port(spec) {
        settings.entry(name).or_insert(value);
    }
    let settings = crate::catalog::validate_settings(&spec.settings, &settings, &model.id)
        .map_err(|e| format!("settings: {e}"))?;

    let ctx = OpenContext {
        host: HOST,
        host_name: None,
        port: vector_port(spec),
        model: model.id.clone(),
        channels: model.channels,
        settings,
        monitor: true,
    };
    let link = spec.serial_link(&model.id, &ctx.settings);
    let mut engine = SpecEngine::new(Arc::new(spec.clone()), ctx)?;
    // Vectors check what is sent, not when: pacing (SPEC.md §2) has its own
    // engine tests.
    engine.pacer = super::pacing::Pacer::default();

    // Bring the session up: connect, and answer any probe the engine sends
    // first, so only the command's own messages are compared.
    let mut cx = Cx::new(0);
    engine.start(&mut cx);
    let started = cx.take();
    check_link(link, &started)?;
    let mut cx = Cx::new(1);
    if started.iter().any(|a| {
        matches!(
            a,
            Action::TcpOpen { .. } | Action::TcpOpenTls { .. } | Action::SerialOpen { .. }
        )
    }) {
        engine.tcp(&mut cx, "device", TcpInput::Connected);
        // A login that waits for the device's prompts is given each in turn
        // (Sharp's user name, then password).
        for _ in 0..8 {
            let Some((_, prompt, _)) = engine.prompt_wait.clone() else {
                break;
            };
            engine.tcp(&mut cx, "device", TcpInput::Data(prompt.into_bytes()));
        }
    }
    if started.iter().any(|a| {
        matches!(
            a,
            Action::WsOpen {
                socket: "device",
                ..
            }
        )
    }) {
        engine.ws(&mut cx, "device", WsInput::Opened);
    }
    // HTTP polls and the probe go one at a time: each answered request lets
    // the next go.
    let mut pending = http_ids(&started);
    // The cookie the spec's session is captured from (`session` when it
    // names none), so `{session.id}` renders as `vector`.
    let session_cookie = engine
        .session
        .as_ref()
        .and_then(|s| s.cookie_name())
        .unwrap_or("session")
        .to_string();
    while let Some(id) = pending.pop() {
        let mut step = Cx::new(1);
        engine.http_response(
            &mut step,
            id,
            // With a session cookie, for a spec whose requests need a
            // session (`transport.session`): its login is answered too.
            Ok(HttpResponse {
                headers: vec![(
                    "set-cookie".into(),
                    format!("{session_cookie}=vector; path=/"),
                )],
                status: 200,
                body: b"{}".to_vec(),
            }),
        );
        pending.extend(http_ids(&step.take()));
    }
    if started.iter().any(|a| matches!(a, Action::UdpSend { .. })) && engine.current.is_some() {
        let probe_reply = super::osc::encode("/probe-reply", &[]);
        engine.datagram(&mut cx, "device", SocketAddr::new(HOST, 1), &probe_reply);
    }
    // Telemetry subscriptions and queries are queued ahead of commands.
    // Complete them with a plain success reply, so the vector sees only its
    // command: the first of these that the transport takes as a reply (its
    // reply_match, where it has one). OSC queries are answered on their own
    // address.
    const SUCCESS: &[&str] = &[
        "200 ok",
        "~01@ok",
        "ACK;",
        "ACK",
        "ack,ok",
        "OK ok",
        "\u{6}",
        "[m]",
        r#"{"jsonrpc":"2.0","id":1}"#,
        "REP ok",
        // LW3: the answer to OPEN is "o- <node>".
        "o- ok",
        "!Done ok",
        // Turtle AV's acknowledgement.
        "[SUCCESS]ok",
        // Lightware LW2: every answer is in round brackets.
        "(ok)",
        // Sony BRAVIA Simple IP control: an answer of sixteen 0.
        "*SAPOWR0000000000000000",
        // LG displays: "[Command2] [Set ID] OK[Data]x", ended by the x.
        "a 01 OK01x",
        // Sharp NEC's ASCII control commands: every response starts with ">".
        ">ok",
        // The Leitch pass-through prompt after each command (Videohub RS-422).
        ">",
        // TEAC's TASCAM protocols: a return command (machine ID 0, then 8F,
        // the software version).
        "08F0100",
        // Shure UHF-R: a REPORT between asterisks.
        "* REPORT ok *",
        // Meyer Sound GALAXY: the answer to :ping.
        ":pong",
    ];
    let line = match &engine.transport {
        super::Transport::LineTcp {
            reply_match: Some(re),
            ..
        }
        | super::Transport::LineUdp {
            reply_match: Some(re),
            ..
        } => {
            // A delimited reply is matched with its markers.
            let wrap = |s: &str| match &engine.transport {
                super::Transport::LineTcp {
                    reply: super::ReplyFraming::Delimited { open, close },
                    ..
                } => format!("{open}{s}{close}"),
                _ => s.to_string(),
            };
            SUCCESS
                .iter()
                .find(|s| re.is_match(&wrap(s)))
                .copied()
                .unwrap_or(SUCCESS[0])
        }
        _ => SUCCESS[0],
    };
    // A block transport's reply ends at a blank line; a delimited one is
    // wrapped in its markers (Shure's `< ... >`, PIXERA's separator-only
    // 0xPX).
    let success = match &engine.transport {
        super::Transport::LineTcp {
            reply: super::ReplyFraming::Delimited { open, close },
            ..
        } => format!("{open}{line}{close}"),
        super::Transport::LineTcp {
            reply: super::ReplyFraming::Block,
            ..
        } => format!("{line}\r\n\r\n"),
        // A headed block whose headers say this first line has more after
        // it (CasparCG's `200 ...` lines up to a blank line, `201 ...` one
        // line of data).
        super::Transport::LineTcp {
            reply: super::ReplyFraming::HeadedBlock(headers),
            ..
        } => {
            let matches = |re: &Option<String>| {
                re.as_deref()
                    .and_then(|re| regex::Regex::new(re).ok())
                    .is_some_and(|re| re.is_match(line))
            };
            if matches(&headers.block) {
                format!("{line}\r\n\r\n")
            } else if matches(&headers.line) {
                format!("{line}\r\nok\r\n")
            } else {
                format!("{line}\r\n")
            }
        }
        // A JSON stream's messages are JSON values: JSON-RPC (Barco Pulse),
        // or a bare {"result": ...} where the reply must start with it
        // (NewBlue Captivate).
        super::Transport::LineTcp {
            reply: super::ReplyFraming::Json,
            reply_match,
            ..
        } => {
            let bare = format!("{{\"result\":{}}}", Value::String(line.into()));
            let rpc = format!(
                "{{\"jsonrpc\":\"2.0\",\"result\":{}}}",
                Value::String(line.into())
            );
            match reply_match {
                Some(re) if !re.is_match(&rpc) && re.is_match(&bare) => bare,
                _ => rpc,
            }
        }
        _ => format!("{line}\r\n"),
    }
    .into_bytes();
    let success = success.as_slice();
    let internal = |e: &SpecEngine| e.current.as_ref().is_some_and(|f| f.id.is_none());
    let mut guard = 0;
    while internal(&engine) && guard < 65536 {
        // Once only telemetry queries are queued, they have nothing to do with
        // the command (which would go ahead of them anyway), so they are
        // dropped: a long poll list, such as WING's 5,344 values, is not
        // answered for every vector.
        if !matches!(engine.transport, super::Transport::Http { .. })
            && engine
                .queue
                .iter()
                .all(|j| j.id.is_none() && j.item.is_some())
        {
            engine.queue.clear();
        }
        let before = engine.queue.len();
        answer_internal(&mut engine, &mut cx, success);
        // An answer that moved nothing on will not move it on later either.
        if internal(&engine) && engine.queue.len() == before {
            break;
        }
        guard += 1;
    }
    if internal(&engine) {
        return Err("the connection sequence did not complete".into());
    }
    cx.take();

    let mut cx = Cx::new(2);
    engine.command(&mut cx, 1, command, &params);
    let actions = cx.take();
    let mut sent = wire(&actions);

    // On a transport whose device answers, a command of several messages
    // sends each after the reply to the one before: the device's reply lets
    // the rest go, and when the vector states more messages than went out at
    // once, what the reply let go is compared too.
    let mut reply_actions = None;
    if let Some(reply) = v.get("device_reply").and_then(Value::as_str) {
        let mut cx = Cx::new(3);
        // A websocket device's reply is one message; any other is bytes on
        // the stream.
        if matches!(engine.transport, super::Transport::Ws { .. }) {
            engine.ws(&mut cx, "device", WsInput::Text(reply.to_string()));
        } else {
            engine.tcp(&mut cx, "device", TcpInput::Data(reply.as_bytes().to_vec()));
        }
        let after = cx.take();
        let stated = expected_wire(&v, main_port(&engine)).map_or(0, |w| w.len());
        if stated > sent.len() {
            sent.extend(wire(&after));
        }
        reply_actions = Some(after);
    }

    if let Some(expected) = expected_wire(&v, main_port(&engine)) {
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

    if let Some(after) = reply_actions {
        done.extend(after.iter().filter_map(|a| match a {
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
                headers: Vec::new(),
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

/// `MEROS_VECTORS`: spec ids, separated by commas, whose vectors alone
/// `every_vector_passes` runs (`MEROS_VECTORS=boinx-mimolive`). Unset or
/// empty, every vector runs.
fn vector_filter() -> Option<Vec<String>> {
    let ids = std::env::var("MEROS_VECTORS").ok()?;
    let ids: Vec<String> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (!ids.is_empty()).then_some(ids)
}

#[test]
fn every_vector_passes() {
    let catalog = Catalog::source_tree();
    let mut files = vector_files();
    if let Some(ids) = vector_filter() {
        files.retain(|f| {
            f.parent()
                .and_then(|d| d.file_name())
                .is_some_and(|d| ids.iter().any(|id| d == id.as_str()))
        });
        assert!(!files.is_empty(), "no vectors for MEROS_VECTORS={ids:?}");
    }
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
    let catalog = Catalog::source_tree();
    let mut covered = std::collections::BTreeSet::new();
    for f in vector_files() {
        let v: Value = crate::yaml::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let Some(command) = v["command"].as_str() else {
            continue;
        };
        covered.insert(format!("{}/{command}", v["spec"].as_str().unwrap()));
    }
    let missing: Vec<String> = catalog
        .devices
        .values()
        .filter(|s| s.implementation == crate::catalog::Implementation::Spec)
        // A command without a message is answered by the spec's native
        // extension, and tested in its session test instead.
        .flat_map(|s| {
            s.commands
                .iter()
                .filter(|(_, c)| c.send.is_some())
                .map(move |(c, _)| format!("{}/{c}", s.id))
        })
        .filter(|k| !covered.contains(k))
        .collect();
    assert!(
        missing.is_empty(),
        "commands without a vector:\n{}",
        missing.join("\n")
    );
}

/// A spec reaching its device over TCP or a serial port (a `serial` block):
/// the engine opened the link the vector's settings choose, so a vector with
/// `link: serial` is played on the serial port.
fn check_link(serial: Option<bool>, started: &[Action]) -> Result<(), String> {
    let opened_serial = started
        .iter()
        .any(|a| matches!(a, Action::SerialOpen { .. }));
    let opened_tcp = started
        .iter()
        .any(|a| matches!(a, Action::TcpOpen { .. } | Action::TcpOpenTls { .. }));
    match serial {
        Some(true) if !opened_serial => Err("link serial did not open the serial port".into()),
        Some(false) if !opened_tcp => Err("link tcp did not open a TCP connection".into()),
        _ => Ok(()),
    }
}

/// A serial port for a spec that declares one, which has no default.
fn serial_port(spec: &crate::catalog::DeviceSpec) -> crate::catalog::Params {
    let mut given = crate::catalog::Params::new();
    if spec.settings.contains_key("serial_port") {
        given.insert("serial_port".into(), "COM1".into());
    }
    given
}

#[test]
fn every_spec_with_telemetry_has_a_telemetry_vector_and_constructs() {
    let catalog = Catalog::source_tree();
    let mut covered = std::collections::BTreeSet::new();
    for f in vector_files() {
        let v: Value = crate::yaml::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        if [
            "inbound",
            "inbound_hex",
            "inbound_http",
            "inbound_ws",
            "inbound_sse",
        ]
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
            host_name: None,
            port: vector_port(spec),
            model: model.id.clone(),
            channels: model.channels,
            // With no value for a required setting (a token), the defaults
            // of the others, as a host opening it would have them.
            settings: crate::catalog::validate_settings(
                &spec.settings,
                &serial_port(spec),
                &model.id,
            )
            .unwrap_or_else(|_| {
                spec.settings
                    .iter()
                    .filter_map(|(k, p)| p.default.clone().map(|d| (k.clone(), d)))
                    .collect()
            }),
            monitor: true,
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
