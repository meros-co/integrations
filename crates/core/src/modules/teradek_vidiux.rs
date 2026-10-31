//! Teradek VidiU X, over the MQTT broker its web interface talks to.
//!
//! The encoder serves MQTT 3.1.1 in WebSocket binary messages at
//! `ws://<encoder>/mqtt`, with the web interface's user name and password
//! (the MIT-licensed Bitfocus Companion module `companion-module-teradek-
//! vidiux`, `src/api.js`). The session is the core's MQTT client (`mqtt.rs`);
//! this module names the topics:
//!
//! - status arrives as JSON on `System/Product` (`name`),
//!   `Session/0/Record/Info` (`State`, `Percent Used`, `Size`, `Used`,
//!   `Available`, `Uptime`, `State Details`) and `Session/0/Stream/0/Info`
//!   (`State`, `Uptime`, `State Details`), subscribed on connecting;
//! - a command is `{}` published at QoS 2, retained, on
//!   `<prefix>/<command>/<milliseconds since 1970>`, the record prefix
//!   `Session/0/Record` and the stream prefix `Session/0/Stream/0`.
//!
//! The client's own bookkeeping (session, server and subscription state)
//! is not kept; topics subscribed by command are kept under `topics`.

use std::net::SocketAddr;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    Action, CommandError, CommandId, Cx, FileInput, HttpResponse, Key, Module, OpenContext,
    RequestId, SseInput, TcpInput, WsInput,
};

use super::mqtt::Mqtt;

const PRODUCT: &str = "System/Product";
const RECORD: &str = "Session/0/Record";
const STREAM: &str = "Session/0/Stream/0";

/// The `stream` command's actions and the command each publishes.
const STREAM_ACTIONS: &[(&str, &str)] = &[
    ("publish", "publish"),
    ("unpublish", "unpublish"),
    ("sharelink_publish", "Sharelink/publish"),
    ("sharelink_unpublish", "Sharelink/unpublish"),
    ("broadcast", "broadcast"),
    ("halt", "halt"),
    ("cancel", "cancel"),
    ("preview", "preview"),
    ("end_preview", "endpreview"),
    ("complete", "complete"),
];

/// The settings the MQTT client is opened with.
fn client_settings(settings: &Params) -> Params {
    let text = |k: &str, d: &str| {
        settings
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or(d)
            .to_string()
    };
    let mut s = Map::new();
    s.insert("transport".into(), json!("ws"));
    s.insert("ws_port".into(), json!(80));
    s.insert("ws_path".into(), json!("/mqtt"));
    s.insert("username".into(), json!(text("username", "admin")));
    s.insert("password".into(), json!(text("password", "admin")));
    s.insert(
        "subscriptions".into(),
        json!(format!("{PRODUCT},{RECORD}/Info,{STREAM}/Info")),
    );
    s
}

/// A string field of a status payload, as the encoder words it: text as
/// it is, a number as its digits.
fn field(doc: &Value, name: &str) -> Option<Value> {
    match doc.get(name)? {
        Value::String(s) => Some(json!(s)),
        Value::Number(n) => Some(json!(n.to_string())),
        _ => None,
    }
}

/// One message as state: a status topic's known fields, or any other
/// topic's payload.
fn message_state(topic: &str, payload: &str) -> Option<Value> {
    let doc = || serde_json::from_str::<Value>(payload).ok();
    let mut out = Map::new();
    let mut put = |section: &str, key: &str, v: Option<Value>| {
        if let Some(v) = v {
            let s = out.entry(section.to_string()).or_insert_with(|| json!({}));
            s[key] = v;
        }
    };
    if topic == PRODUCT {
        let d = doc()?;
        put("device", "name", field(&d, "name"));
    } else if topic == format!("{RECORD}/Info") {
        let d = doc()?;
        for (key, name) in [
            ("state", "State"),
            ("percent_used", "Percent Used"),
            ("size", "Size"),
            ("used", "Used"),
            ("available", "Available"),
            ("uptime", "Uptime"),
            ("details", "State Details"),
        ] {
            put("recording", key, field(&d, name));
        }
    } else if topic == format!("{STREAM}/Info") {
        let d = doc()?;
        for (key, name) in [
            ("state", "State"),
            ("uptime", "Uptime"),
            ("details", "State Details"),
        ] {
            put("streaming", key, field(&d, name));
        }
    } else {
        return Some(json!({"topics": { topic: { "payload": payload } }}));
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// The MQTT client's state patch, as this device's state: only messages.
fn transform(patch: &Value) -> Option<Value> {
    let topics = patch.get("topics")?.as_object()?;
    let mut merged = json!({});
    for (topic, entry) in topics {
        let Some(payload) = entry.get("payload").and_then(Value::as_str) else {
            continue;
        };
        if let Some(p) = message_state(topic, payload) {
            crate::session::merge_patch(&mut merged, &p);
        }
    }
    (merged.as_object().is_some_and(|m| !m.is_empty())).then_some(merged)
}

pub(crate) struct VidiuX {
    client: Mqtt,
}

impl VidiuX {
    pub(crate) fn new(ctx: OpenContext) -> Result<VidiuX, String> {
        let mut inner = ctx.clone();
        inner.settings = client_settings(&ctx.settings);
        Ok(VidiuX {
            client: Mqtt::new(inner)?,
        })
    }

    /// Run a client callback, keeping its state for this device's.
    fn relay(&mut self, cx: &mut Cx, f: impl FnOnce(&mut Mqtt, &mut Cx)) {
        let mut inner = Cx::at(cx.now(), cx.unix_millis());
        f(&mut self.client, &mut inner);
        for action in inner.take() {
            match action {
                Action::State(patch) => {
                    if let Some(p) = transform(&patch) {
                        cx.state(p);
                    }
                }
                other => cx.push(other),
            }
        }
    }

    /// Publish `{}` on `<topic>/<now>` at QoS 2, retained.
    fn send_command(&mut self, cx: &mut Cx, id: CommandId, topic: &str) {
        let params: Params = json!({
            "topic": format!("{topic}/{}", cx.unix_millis()),
            "payload": "{}",
            "qos": 2,
            "retain": true,
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        self.relay(cx, |c, cx| c.command(cx, id, "publish", &params));
    }
}

fn invalid(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
    cx.complete(
        id,
        Err(CommandError::InvalidParams {
            message: message.into(),
        }),
    );
}

impl Module for VidiuX {
    fn start(&mut self, cx: &mut Cx) {
        self.relay(cx, |c, cx| c.start(cx));
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let text = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
        match name {
            "start_recording" => self.send_command(cx, id, &format!("{RECORD}/start")),
            "stop_recording" => self.send_command(cx, id, &format!("{RECORD}/stop")),
            "stream" => {
                let action = text("action");
                match STREAM_ACTIONS.iter().find(|(a, _)| *a == action) {
                    Some((_, wire)) => self.send_command(cx, id, &format!("{STREAM}/{wire}")),
                    None => invalid(cx, id, format!("unknown stream action '{action}'")),
                }
            }
            "send_command" => {
                let topic = text("topic").to_string();
                if topic.is_empty() || topic.contains(['#', '+']) {
                    return invalid(
                        cx,
                        id,
                        "the command topic is a topic name without wildcards",
                    );
                }
                self.send_command(cx, id, &topic);
            }
            "subscribe" => {
                let params: Params = json!({"filter": text("filter"), "qos": 0})
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                self.relay(cx, |c, cx| c.command(cx, id, "subscribe", &params));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        self.relay(cx, |c, cx| c.datagram(cx, socket, from, data));
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.relay(cx, |c, cx| c.socket_error(cx, socket, message));
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.relay(cx, |c, cx| c.tcp(cx, socket, input));
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.relay(cx, |c, cx| c.ws(cx, socket, input));
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.relay(cx, |c, cx| c.http_response(cx, id, result));
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.relay(cx, |c, cx| c.sse(cx, stream, input));
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.relay(cx, |c, cx| c.file(cx, file, input));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        self.relay(cx, |c, cx| c.timer(cx, key));
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.relay(cx, |c, cx| c.stream_watch(cx, stream, watching));
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.relay(cx, |c, cx| c.stop(cx));
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;
    use crate::session::merge_patch;

    const WS: Key = "mqtt";

    fn module(settings: Value) -> VidiuX {
        VidiuX::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "vidiu-x".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        })
        .unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::WsSendBinary { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    /// A QoS 0 PUBLISH of `payload` on `topic`.
    fn publish(topic: &str, payload: &str) -> Vec<u8> {
        let mut body = (topic.len() as u16).to_be_bytes().to_vec();
        body.extend_from_slice(topic.as_bytes());
        body.extend_from_slice(payload.as_bytes());
        let mut out = vec![0x30];
        let mut n = body.len();
        loop {
            let mut b = (n % 128) as u8;
            n /= 128;
            if n > 0 {
                b |= 0x80;
            }
            out.push(b);
            if n == 0 {
                break;
            }
        }
        out.extend(body);
        out
    }

    /// Opened, connected and accepted: what it sent and did.
    fn ready(m: &mut VidiuX) -> Vec<Action> {
        let mut cx = Cx::at(0, 1_700_000_000_000);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a,
            Action::WsOpen { request, .. } if request.url == "ws://127.0.0.1:80/mqtt")));
        let mut cx = Cx::at(1, 1_700_000_000_001);
        m.ws(&mut cx, WS, WsInput::Opened);
        let connect = sent(&cx.take());
        // CONNECT with a user name and a password (flags C2), keep alive 30.
        assert_eq!(connect[0][0], 0x10);
        assert_eq!(connect[0][9], 0xc2);
        let mut cx = Cx::at(2, 1_700_000_000_002);
        m.ws(&mut cx, WS, WsInput::Binary(vec![0x20, 2, 0, 0]));
        cx.take()
    }

    #[test]
    fn the_status_topics_are_subscribed_and_read_into_state() {
        let mut m = module(json!({"username": "admin", "password": "secret"}));
        let a = ready(&mut m);
        let subscribe = sent(&a);
        let text = String::from_utf8_lossy(&subscribe[0]);
        assert!(text.contains("System/Product"));
        assert!(text.contains("Session/0/Record/Info"));
        assert!(text.contains("Session/0/Stream/0/Info"));
        // The client's own state (version, session) is not this device's.
        assert_eq!(state(&a), json!({}));

        let mut cx = Cx::at(3, 1_700_000_000_003);
        m.ws(
            &mut cx,
            WS,
            WsInput::Binary(publish(
                "Session/0/Record/Info",
                r#"{"State":"Recording","Percent Used":"12 %","Size":"29.7 G","Used":"3.6 G","Available":"26.1 G","Uptime":"00:01:02"}"#,
            )),
        );
        m.ws(
            &mut cx,
            WS,
            WsInput::Binary(publish(
                "Session/0/Stream/0/Info",
                r#"{"State":"Live","Uptime":"00:10:00","State Details":"Connected"}"#,
            )),
        );
        m.ws(
            &mut cx,
            WS,
            WsInput::Binary(publish("System/Product", r#"{"name":"VidiU X"}"#)),
        );
        m.ws(&mut cx, WS, WsInput::Binary(publish("Network/Info", "up")));
        assert_eq!(
            state(&cx.take()),
            json!({
                "recording": {"state": "Recording", "percent_used": "12 %", "size": "29.7 G",
                              "used": "3.6 G", "available": "26.1 G", "uptime": "00:01:02"},
                "streaming": {"state": "Live", "uptime": "00:10:00", "details": "Connected"},
                "device": {"name": "VidiU X"},
                "topics": {"Network/Info": {"payload": "up"}}
            })
        );
    }

    #[test]
    fn commands_publish_on_a_timed_topic_at_qos_2_retained() {
        let mut m = module(json!({}));
        ready(&mut m);
        let mut cx = Cx::at(10, 1_700_000_000_123);
        let p: Params = json!({"action": "end_preview"})
            .as_object()
            .unwrap()
            .clone();
        m.command(&mut cx, 7, "stream", &p);
        let out = sent(&cx.take());
        // PUBLISH, QoS 2 and retain (flags 0101), the topic, a packet id, {}.
        assert_eq!(out[0][0], 0x35);
        let text = String::from_utf8_lossy(&out[0]);
        assert!(text.contains("Session/0/Stream/0/endpreview/1700000000123"));
        assert!(text.ends_with("{}"));

        let mut cx = Cx::at(11, 1_700_000_000_200);
        m.command(&mut cx, 8, "start_recording", &Params::new());
        let text = String::from_utf8_lossy(&sent(&cx.take())[0]).to_string();
        assert!(text.contains("Session/0/Record/start/1700000000200"));

        let mut cx = Cx::at(11, 1_700_000_000_300);
        m.command(&mut cx, 10, "stop_recording", &Params::new());
        let text = String::from_utf8_lossy(&sent(&cx.take())[0]).to_string();
        assert!(text.contains("Session/0/Record/stop/1700000000300"));

        let mut cx = Cx::at(11, 1_700_000_000_400);
        let p: Params = json!({"topic": "Session/0/Stream/0/Sharelink/publish"})
            .as_object()
            .unwrap()
            .clone();
        m.command(&mut cx, 11, "send_command", &p);
        let text = String::from_utf8_lossy(&sent(&cx.take())[0]).to_string();
        assert!(text.contains("Session/0/Stream/0/Sharelink/publish/1700000000400"));

        // A subscription: SUBSCRIBE (82) for the filter at QoS 0.
        let mut cx = Cx::at(11, 0);
        let p: Params = json!({"filter": "Session/0/#"})
            .as_object()
            .unwrap()
            .clone();
        m.command(&mut cx, 12, "subscribe", &p);
        let out = sent(&cx.take());
        assert_eq!(out[0][0], 0x82);
        assert!(String::from_utf8_lossy(&out[0]).contains("Session/0/#"));

        let mut cx = Cx::at(12, 0);
        let p: Params = json!({"action": "explode"}).as_object().unwrap().clone();
        m.command(&mut cx, 9, "stream", &p);
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::Complete {
                id: 9,
                result: Err(CommandError::InvalidParams { .. })
            }
        )));
    }
}
