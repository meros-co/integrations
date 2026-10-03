//! NovaStar H series video wall splicers over the H Series OpenAPI.
//!
//! Protocol from NovaStar's "H Series OpenAPI" documentation (English pages
//! on openapi.novastar.tech, read 2026-10-03; section numbers are its
//! "Instructions" page's).
//!
//! - Every request is an HTTP POST of JSON to
//!   `http://<splicer>:8000/open/api/<endpoint>` (§2, §5), with the body
//!   `{"body": {...}, "sign": ..., "pId": ..., "timeStamp": ...}` (§4).
//!   Unencrypted, `sign` is Base64 of the hexadecimal MD5 of the timestamp
//!   followed by the pId, and the timestamp is the current time in
//!   milliseconds. The splicer refuses a wrong time (error 11), so the module
//!   needs the wall clock: it takes the time once, when the device is opened,
//!   and adds the session's own clock to it. That is the one place the module
//!   reads a clock; tests give it a fixed one.
//! - Replies are `{"status", "msg", "sign", "body"}`; status 0 is success
//!   (one documented example spells the key `"status "`, which is read too).
//!   Other statuses fail the command with that code.
//! - Commands come from [`ENDPOINTS`]: each copies its parameters into the
//!   body under the documented field names, adds `deviceId`, and merges a
//!   `body` parameter's fields where the endpoint takes nested structures.
//!   `call` sends any endpoint with a body given whole.
//! - The splicer does not push in any documented form, so the module polls:
//!   the initialisation status (which is also the liveness check), the
//!   screen list, and each listed screen's current preset.
//!
//! Opened for commands only (`monitor` false), the module asks only the
//! initialisation status at the poll interval, as its liveness check, and
//! reads no screens or presets. Each answered request reports its round trip.

use std::collections::HashMap;

use base64::Engine;
use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

use super::novastar_h_commands::{Endpoint, ENDPOINTS};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, Outcome, RequestId,
};

pub(crate) const DEFAULT_PORT: u16 = 8000;
const POLL: Key = "poll";
const TIMEOUT: Millis = 5_000;

/// What a request is for.
#[derive(Debug, Clone)]
enum Purpose {
    Command(CommandId),
    InitStatus,
    Screens,
    Preset(i64),
}

#[derive(Debug, Clone)]
struct Pending {
    purpose: Purpose,
    sent_at: Millis,
}

pub(crate) struct NovastarH {
    base: String,
    p_id: String,
    /// Unix time in milliseconds when the session's clock read 0.
    epoch: u64,
    poll_every: Millis,
    monitor: bool,
    next_id: RequestId,
    pending: HashMap<RequestId, Pending>,
    connected: Option<bool>,
}

/// Base64 of the hexadecimal MD5 of `time` followed by `p_id` (§4, unencrypted).
pub(crate) fn sign(time: &str, p_id: &str) -> String {
    let digest = Md5::digest(format!("{time}{p_id}").as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    base64::engine::general_purpose::STANDARD.encode(hex)
}

/// The request envelope for a business body at `time` (Unix milliseconds).
pub(crate) fn envelope(body: Value, time: u64, p_id: &str) -> Value {
    let time = time.to_string();
    json!({
        "body": body,
        "sign": sign(&time, p_id),
        "pId": p_id,
        "timeStamp": time,
    })
}

/// The business body of a table command.
pub(crate) fn business_body(endpoint: &Endpoint, params: &Params) -> Result<Value, CommandError> {
    let mut body = Map::new();
    if endpoint.device {
        body.insert(
            "deviceId".into(),
            params.get("device_id").cloned().unwrap_or(json!(0)),
        );
    }
    for (param, field) in endpoint.fields {
        if let Some(v) = params.get(*param) {
            body.insert((*field).into(), v.clone());
        }
    }
    if endpoint.body {
        match params.get("body") {
            None | Some(Value::Null) => {}
            Some(Value::Object(extra)) => {
                for (k, v) in extra {
                    body.insert(k.clone(), v.clone());
                }
            }
            Some(_) => {
                return Err(CommandError::InvalidParams {
                    message: "'body' must be a JSON object".into(),
                })
            }
        }
    }
    Ok(Value::Object(body))
}

/// A reply's status and body; `Err` when it is not the documented envelope.
fn read_reply(bytes: &[u8]) -> Result<(i64, String, Value), String> {
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("reply is not JSON: {e}"))?;
    let status = doc
        .get("status")
        .or_else(|| doc.get("status "))
        .and_then(Value::as_i64)
        .ok_or("reply has no status")?;
    let msg = doc
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok((status, msg, doc.get("body").cloned().unwrap_or(Value::Null)))
}

impl NovastarH {
    pub(crate) fn new(ctx: OpenContext) -> NovastarH {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        NovastarH::with(ctx, epoch)
    }

    fn with(ctx: OpenContext, epoch: u64) -> NovastarH {
        let s = &ctx.settings;
        let port = ctx.port.unwrap_or(DEFAULT_PORT);
        NovastarH {
            base: format!(
                "http://{}/open/api/",
                std::net::SocketAddr::new(ctx.host, port)
            ),
            p_id: s
                .get("p_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            epoch,
            poll_every: s.get("poll_ms").and_then(Value::as_u64).unwrap_or(5_000),
            monitor: ctx.monitor,
            next_id: 1,
            pending: HashMap::new(),
            connected: None,
        }
    }

    fn post(&mut self, cx: &mut Cx, path: &str, body: Value, purpose: Purpose) {
        let id = self.next_id;
        self.next_id += 1;
        let request = envelope(body, self.epoch + cx.now(), &self.p_id);
        cx.http(
            id,
            HttpRequest {
                method: "POST",
                url: format!("{}{path}", self.base),
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: Some(request.to_string().into_bytes()),
                timeout: Some(TIMEOUT),
                accept_invalid_certs: false,
                digest: None,
            },
        );
        self.pending.insert(
            id,
            Pending {
                purpose,
                sent_at: cx.now(),
            },
        );
    }

    fn poll(&mut self, cx: &mut Cx) {
        self.post(cx, "main/initStatus", json!({}), Purpose::InitStatus);
        if self.monitor {
            self.post(
                cx,
                "screen/readList",
                json!({"deviceId": 0}),
                Purpose::Screens,
            );
        }
    }

    fn set_connected(&mut self, cx: &mut Cx, up: bool, reason: &str) {
        if self.connected == Some(up) {
            return;
        }
        self.connected = Some(up);
        cx.connection(if up {
            Connection::Connected
        } else {
            Connection::Disconnected {
                reason: reason.into(),
            }
        });
    }
}

impl Module for NovastarH {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.poll(cx);
        if self.poll_every > 0 {
            cx.set_timer(POLL, self.poll_every);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "call" {
            let path = params.get("path").and_then(Value::as_str).unwrap_or("");
            let path = path.trim_start_matches('/').to_string();
            let body = params.get("body").cloned().unwrap_or(json!({}));
            self.post(cx, &path, body, Purpose::Command(id));
            return;
        }
        let Some(endpoint) = ENDPOINTS.iter().find(|e| e.name == name) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        };
        match business_body(endpoint, params) {
            Ok(body) => self.post(cx, endpoint.path, body, Purpose::Command(id)),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some(pending) = self.pending.remove(&id) else {
            return;
        };
        let response = match result {
            Ok(r) => r,
            Err(reason) => {
                self.set_connected(cx, false, &reason);
                if let Purpose::Command(cid) = pending.purpose {
                    cx.complete(cid, Err(CommandError::Transport { message: reason }));
                }
                return;
            }
        };
        cx.alive();
        cx.round_trip(cx.now().saturating_sub(pending.sent_at));
        self.set_connected(cx, true, "");
        let parsed = if response.status == 200 {
            read_reply(&response.body)
        } else {
            Err(format!("HTTP {}", response.status))
        };
        match pending.purpose {
            Purpose::Command(cid) => match parsed {
                Ok((0, _, body)) => cx.complete(cid, Ok(Outcome::Value { value: body })),
                Ok((status, msg, _)) => cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: Some(status.to_string()),
                        message: if msg.is_empty() {
                            format!("status {status}")
                        } else {
                            msg
                        },
                    }),
                ),
                Err(message) => cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: None,
                        message,
                    }),
                ),
            },
            Purpose::InitStatus => {
                if let Ok((0, _, body)) = parsed {
                    let mut device = Map::new();
                    if let Some(v) = body.get("initStatus").and_then(Value::as_i64) {
                        device.insert("initialized".into(), json!(v == 1));
                    }
                    if let Some(v) = body.get("sn").and_then(Value::as_str) {
                        device.insert("serial".into(), json!(v));
                    }
                    if let Some(v) = body.get("softwareVersion").and_then(Value::as_str) {
                        device.insert("software_version".into(), json!(v));
                    }
                    if let Some(v) = body.get("mainModelId").and_then(Value::as_i64) {
                        device.insert("main_model_id".into(), json!(v));
                    }
                    if !device.is_empty() {
                        cx.state(json!({ "device": device }));
                    }
                }
            }
            Purpose::Screens => {
                if let Ok((0, _, body)) = parsed {
                    let screens = body
                        .get("screens")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mut patch = Map::new();
                    for screen in &screens {
                        let Some(sid) = screen.get("screenId").and_then(Value::as_i64) else {
                            continue;
                        };
                        if let Some(name) = screen.get("name") {
                            patch.insert(sid.to_string(), json!({ "name": name }));
                        }
                        self.post(
                            cx,
                            "preset/readPlay",
                            json!({"deviceId": 0, "screenId": sid}),
                            Purpose::Preset(sid),
                        );
                    }
                    if !patch.is_empty() {
                        cx.state(json!({ "screens": patch }));
                    }
                }
            }
            Purpose::Preset(sid) => {
                if let Ok((0, _, body)) = parsed {
                    if let Some(p) = body.get("presetId").and_then(Value::as_i64) {
                        cx.state(json!({ "screens": { sid.to_string(): { "preset": p } } }));
                    }
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == POLL {
            self.poll(cx);
            if self.poll_every > 0 {
                cx.set_timer(POLL, self.poll_every);
            }
        } else {
            cx.log(Level::Debug, format!("NovaStar H: unknown timer {key}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn context(monitor: bool) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 228)),
            port: None,
            model: "h5".into(),
            channels: None,
            settings: json!({"p_id": "YmRj", "poll_ms": 5000})
                .as_object()
                .unwrap()
                .clone(),
            monitor,
        }
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((
                    *id,
                    request.url.clone(),
                    serde_json::from_slice(request.body.as_ref().unwrap()).unwrap(),
                )),
                _ => None,
            })
            .collect()
    }

    fn reply(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            body: body.to_string().into_bytes(),
        })
    }

    /// Both worked examples of the documentation's pages.
    #[test]
    fn signatures_match_the_documented_examples() {
        assert_eq!(
            sign("1689586062335", "YmRj"),
            "YjlhMWRmZTVlNzJhYTg4MTgzMDFhNTdlOWE0NjMyNDc="
        );
        assert_eq!(
            sign("1689586062335", "ZTlj"),
            "Y2YxNjE4NjRmZTFlOThhODIxMDNhOWY2YmU4MzU5ODk="
        );
    }

    #[test]
    fn a_command_is_enveloped_signed_and_timed() {
        let mut m = NovastarH::with(context(true), 1_689_586_062_000);
        let mut cx = Cx::new(335);
        m.command(
            &mut cx,
            7,
            "screen_ftb",
            json!({"device_id": 0, "screen_id": 1, "type": 0, "seconds": 2})
                .as_object()
                .unwrap(),
        );
        let sent = requests(&cx.take());
        assert_eq!(sent.len(), 1);
        let (_, url, body) = &sent[0];
        assert_eq!(url, "http://192.168.10.228:8000/open/api/screen/ftb");
        assert_eq!(
            body,
            &json!({
                "body": {"deviceId": 0, "screenId": 1, "type": 0, "time": 2},
                "sign": "YjlhMWRmZTVlNzJhYTg4MTgzMDFhNTdlOWE0NjMyNDc=",
                "pId": "YmRj",
                "timeStamp": "1689586062335"
            })
        );
    }

    #[test]
    fn nested_bodies_are_merged_and_call_sends_any_path() {
        let ep = ENDPOINTS
            .iter()
            .find(|e| e.name == "set_layer_source")
            .unwrap();
        let body = business_body(
            ep,
            json!({"device_id": 0, "screen_id": 0, "layer_id": 3, "body": {"source": {"sourceType": 1, "inputId": 4}}})
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({"deviceId": 0, "screenId": 0, "layerId": 3, "source": {"sourceType": 1, "inputId": 4}})
        );
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "call",
            json!({"path": "/ipc/readList", "body": {"deviceId": 0}})
                .as_object()
                .unwrap(),
        );
        let sent = requests(&cx.take());
        assert_eq!(
            sent[0].1,
            "http://192.168.10.228:8000/open/api/ipc/readList"
        );
        assert_eq!(sent[0].2["body"], json!({"deviceId": 0}));
    }

    #[test]
    fn replies_complete_commands_and_report_round_trips() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "load_preset",
            json!({"device_id": 0, "screen_id": 0, "preset_id": 1})
                .as_object()
                .unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(25);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 0, "msg": "Success", "sign": "", "body": ""})),
        );
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(25)));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value { value: json!("") })
        }));

        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            2,
            "get_screen",
            json!({"device_id": 0, "screen_id": 9}).as_object().unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(40);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 404, "msg": "The screen does not exist.", "body": ""})),
        );
        assert!(cx.take().iter().any(|a| matches!(a,
            Action::Complete { id: 2, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "404")));
    }

    #[test]
    fn the_poll_reads_screens_and_their_presets() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sent = requests(&cx.take());
        assert_eq!(sent.len(), 2);
        let screens = sent
            .iter()
            .find(|s| s.1.ends_with("screen/readList"))
            .unwrap()
            .0;
        let init = sent
            .iter()
            .find(|s| s.1.ends_with("main/initStatus"))
            .unwrap()
            .0;
        let mut cx = Cx::new(10);
        // The documented initStatus example spells its status key "status ".
        m.http_response(&mut cx, init, reply(json!({"body": {"initStatus": 1, "mainModelId": 29962, "sn": "FFFFFFFFFFFFFFFF", "softwareVersion": "1.9.7.0.S1.T1"}, "msg": "Success", "sign": "", "status ": 0})));
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"device": {"initialized": true, "serial": "FFFFFFFFFFFFFFFF", "software_version": "1.9.7.0.S1.T1", "main_model_id": 29962}}))));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let mut cx = Cx::new(20);
        m.http_response(&mut cx, screens, reply(json!({"body": {"deviceId": 0, "screens": [{"createTime": "1757509392.7567859", "name": "Screen 1", "screenId": 0}]}, "msg": "", "sign": "", "status": 0})));
        let a = cx.take();
        assert!(a.contains(&Action::State(
            json!({"screens": {"0": {"name": "Screen 1"}}})
        )));
        let preset = requests(&a);
        assert_eq!(preset.len(), 1);
        assert_eq!(preset[0].2["body"], json!({"deviceId": 0, "screenId": 0}));
        let mut cx = Cx::new(30);
        m.http_response(&mut cx, preset[0].0, reply(json!({"body": {"deviceId": 0, "presetId": 1, "screenId": 0}, "msg": "Success", "sign": "", "status": 0})));
        assert!(cx
            .take()
            .contains(&Action::State(json!({"screens": {"0": {"preset": 1}}}))));
    }

    #[test]
    fn opened_for_commands_only_it_asks_only_the_init_status() {
        let mut m = NovastarH::with(context(false), 0);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sent = requests(&cx.take());
        assert_eq!(sent.len(), 1);
        assert!(sent[0].1.ends_with("main/initStatus"));
        // Commands still work.
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "list_screens",
            json!({"device_id": 0}).as_object().unwrap(),
        );
        assert_eq!(requests(&cx.take()).len(), 1);
    }

    #[test]
    fn every_spec_command_has_an_endpoint() {
        let spec: Value =
            serde_yaml::from_str(include_str!("../../../../specs/novastar-h.yaml")).unwrap();
        for name in spec["commands"].as_object().unwrap().keys() {
            assert!(
                name == "call" || ENDPOINTS.iter().any(|e| e.name == name),
                "{name}"
            );
        }
        assert_eq!(
            ENDPOINTS.len() + 1,
            spec["commands"].as_object().unwrap().len()
        );
    }
}
