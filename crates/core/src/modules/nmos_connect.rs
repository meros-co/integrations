//! IS-05 connection management from an NMOS registry, for `nmos-registry`.
//!
//! A registry's Query API (AMWA IS-04) is read only: connections are made on
//! each Node's Connection API (AMWA IS-05), whose address the registry gives
//! in the Device's `controls`. This finds it and makes the request, so a
//! consumer routes from the registry alone, as an NMOS controller does:
//!
//! 1. `GET /x-nmos/query/<version>/receivers/<id>` (or `senders`) on the
//!    registry: the resource's `device_id`.
//! 2. `GET /x-nmos/query/<version>/devices/<device_id>`: its `controls`, of
//!    which the one whose `type` is `urn:x-nmos:control:sr-ctrl/<version>`
//!    (the highest version, when there are several) is the Connection API's
//!    base address (IS-04 v1.3 device.json; IS-05 v1.1 "Interoperability -
//!    IS-04").
//! 3. For `connect_receiver`, the Sender's transport file: the Sender's
//!    `manifest_href` (IS-04 sender.json), read as it is; a Sender without one
//!    (a transport needing no file) is connected by its id alone.
//! 4. `PATCH <href>single/receivers/<id>/staged` (or `senders`) with the
//!    staged parameters and an immediate activation (IS-05
//!    ConnectionAPI.raml, receiver-stage-schema.json), or `GET
//!    <href>single/.../active` for a read.
//!
//! The addresses come from the registry's own records, never from the
//! caller. The registry's bearer token, where one is set, goes with every
//! request, the Node's included: an IS-10 deployment shares its
//! authorization server. Requests go one command at a time per command, each
//! waiting up to five seconds.

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Cx, HttpRequest, HttpResponse, Millis, Outcome, RequestId,
};

/// The commands this answers (SPEC.md, "Native extensions").
pub(crate) const COMMANDS: [&str; 6] = [
    "connect_receiver",
    "disconnect_receiver",
    "stage_receiver",
    "stage_sender",
    "get_receiver_active",
    "get_sender_active",
];

/// Request ids at and above this are this helper's.
pub(crate) const IDS: RequestId = 1 << 49;
const TIMEOUT: Millis = 5_000;
const SR_CTRL: &str = "urn:x-nmos:control:sr-ctrl/";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    Receiver,
    Sender,
}

impl Side {
    fn plural(self) -> &'static str {
        match self {
            Side::Receiver => "receivers",
            Side::Sender => "senders",
        }
    }
}

/// What a command does once the Connection API is found.
#[derive(Debug, Clone, PartialEq)]
enum Action {
    /// PATCH staged with this body.
    Stage(Value),
    /// GET active.
    Active,
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// The resource from the registry, for its Device.
    Resource,
    /// The Device, for its Connection API.
    Device,
    /// The Sender to connect, for its transport file's address.
    SenderResource,
    /// The Sender's transport file.
    Manifest,
    /// The request to the Node.
    Node,
}

#[derive(Debug)]
struct Job {
    command: CommandId,
    side: Side,
    id: String,
    action: Action,
    /// connect_receiver: the Sender whose transport file goes with it.
    sender: Option<String>,
    /// The Connection API's base address, once found.
    href: Option<String>,
    step: Step,
    request: RequestId,
}

pub(crate) struct Connector {
    /// `scheme://host:port` of the Query API.
    origin: String,
    version: String,
    token: Option<String>,
    jobs: Vec<Job>,
    next_id: RequestId,
}

fn text(params: &Params, name: &str) -> Option<String> {
    params.get(name).and_then(Value::as_str).map(String::from)
}

fn device_error(message: impl Into<String>) -> CommandError {
    CommandError::DeviceError {
        code: None,
        message: message.into(),
    }
}

/// The Connection API's base address among a Device's controls: the
/// sr-ctrl control of the highest version, ending in a slash.
fn connection_href(device: &Value) -> Option<String> {
    let controls = device.get("controls")?.as_array()?;
    let best = controls
        .iter()
        .filter_map(|c| {
            let kind = c.get("type")?.as_str()?;
            let version = kind.strip_prefix(SR_CTRL)?;
            let href = c.get("href")?.as_str()?.trim();
            let parsed: Vec<u32> = version
                .trim_start_matches('v')
                .split('.')
                .filter_map(|n| n.parse().ok())
                .collect();
            (href.starts_with("http://") || href.starts_with("https://"))
                .then(|| (parsed, href.to_string()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))?;
    let mut href = best.1;
    if !href.ends_with('/') {
        href.push('/');
    }
    Some(href)
}

impl Connector {
    pub(crate) fn new(origin: String, version: String, token: Option<String>) -> Connector {
        Connector {
            origin,
            version,
            token,
            jobs: Vec::new(),
            next_id: IDS,
        }
    }

    pub(crate) fn answers(name: &str) -> bool {
        COMMANDS.contains(&name)
    }

    pub(crate) fn owns(id: RequestId) -> bool {
        id >= IDS
    }

    fn headers(&self, json_body: bool) -> Vec<(String, String)> {
        let mut h: Vec<(String, String)> = self
            .token
            .iter()
            .map(|t| ("Authorization".to_string(), format!("Bearer {t}")))
            .collect();
        if json_body {
            h.push(("Content-Type".into(), "application/json".into()));
        }
        h
    }

    fn send(
        &mut self,
        cx: &mut Cx,
        method: &'static str,
        url: String,
        body: Option<Value>,
    ) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        let headers = self.headers(body.is_some());
        cx.http(
            id,
            HttpRequest {
                method,
                url,
                headers,
                body: body.map(|b| b.to_string().into_bytes()),
                body_stream: None,
                timeout: Some(TIMEOUT),
                accept_invalid_certs: true,
                digest: None,
            },
        );
        id
    }

    fn query(&mut self, cx: &mut Cx, path: &str) -> RequestId {
        let url = format!("{}/x-nmos/query/{}/{path}", self.origin, self.version);
        self.send(cx, "GET", url, None)
    }

    pub(crate) fn command(&mut self, cx: &mut Cx, command: CommandId, name: &str, params: &Params) {
        let receiver = text(params, "receiver_id");
        let sender = text(params, "sender_id");
        let staged = params.get("staged").cloned();
        let immediate = json!({"mode": "activate_immediate"});
        let (side, id, action, with_sender) = match name {
            "connect_receiver" => {
                let mut body =
                    json!({"sender_id": sender, "master_enable": true, "activation": immediate});
                if sender.is_none() {
                    body["sender_id"] = Value::Null;
                }
                (
                    Side::Receiver,
                    receiver,
                    Action::Stage(body),
                    sender.clone(),
                )
            }
            "disconnect_receiver" => (
                Side::Receiver,
                receiver,
                Action::Stage(
                    json!({"sender_id": null, "master_enable": false, "activation": immediate}),
                ),
                None,
            ),
            "stage_receiver" => (
                Side::Receiver,
                receiver,
                Action::Stage(staged.unwrap_or(Value::Null)),
                None,
            ),
            "stage_sender" => (
                Side::Sender,
                sender,
                Action::Stage(staged.unwrap_or(Value::Null)),
                None,
            ),
            "get_receiver_active" => (Side::Receiver, receiver, Action::Active, None),
            "get_sender_active" => (Side::Sender, sender, Action::Active, None),
            _ => {
                cx.complete(
                    command,
                    Err(CommandError::UnknownCommand {
                        command: name.into(),
                    }),
                );
                return;
            }
        };
        let Some(id) = id else {
            cx.complete(
                command,
                Err(CommandError::InvalidParams {
                    message: format!(
                        "{name} needs the {} id",
                        if side == Side::Receiver {
                            "receiver"
                        } else {
                            "sender"
                        }
                    ),
                }),
            );
            return;
        };
        if let Action::Stage(body) = &action {
            if !body.is_object() {
                cx.complete(
                    command,
                    Err(CommandError::InvalidParams {
                        message: "staged must be a JSON object".into(),
                    }),
                );
                return;
            }
        }
        let request = self.query(cx, &format!("{}/{id}", side.plural()));
        self.jobs.push(Job {
            command,
            side,
            id,
            action,
            sender: with_sender,
            href: None,
            step: Step::Resource,
            request,
        });
    }

    /// An answer to one of this helper's requests.
    pub(crate) fn http_response(
        &mut self,
        cx: &mut Cx,
        request: RequestId,
        result: Result<HttpResponse, String>,
    ) {
        let Some(at) = self.jobs.iter().position(|j| j.request == request) else {
            return;
        };
        let mut job = self.jobs.remove(at);
        match self.advance(cx, &mut job, result) {
            Ok(Some(next)) => {
                job.request = next;
                self.jobs.push(job);
            }
            Ok(None) => {}
            Err(e) => cx.complete(job.command, Err(e)),
        }
    }

    /// Take the answer to the job's current step: the next request, or the
    /// command completed (`None`).
    fn advance(
        &mut self,
        cx: &mut Cx,
        job: &mut Job,
        result: Result<HttpResponse, String>,
    ) -> Result<Option<RequestId>, CommandError> {
        let response = result.map_err(|message| CommandError::Transport { message })?;
        let ok = (200..300).contains(&response.status);
        let what = match job.step {
            Step::Resource => format!("the registry's {} {}", job.side.plural(), job.id),
            Step::Device => "the registry's device".into(),
            Step::SenderResource => "the registry's sender".into(),
            Step::Manifest => "the sender's transport file".into(),
            Step::Node => "the node's Connection API".into(),
        };
        if !ok {
            let body = String::from_utf8_lossy(&response.body);
            return Err(CommandError::DeviceError {
                code: Some(response.status.to_string()),
                message: format!("{what} answered HTTP {}: {}", response.status, body.trim()),
            });
        }
        match job.step {
            Step::Resource => {
                let doc: Value = serde_json::from_slice(&response.body)
                    .map_err(|_| device_error(format!("{what} is not JSON")))?;
                let device = doc
                    .get("device_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| device_error(format!("{what} names no device_id")))?;
                job.step = Step::Device;
                Ok(Some(self.query(cx, &format!("devices/{device}"))))
            }
            Step::Device => {
                let doc: Value = serde_json::from_slice(&response.body)
                    .map_err(|_| device_error(format!("{what} is not JSON")))?;
                let href = connection_href(&doc).ok_or_else(|| {
                    device_error("the device lists no Connection API (no urn:x-nmos:control:sr-ctrl control)")
                })?;
                job.href = Some(href);
                match &job.sender {
                    Some(sender) => {
                        job.step = Step::SenderResource;
                        let sender = sender.clone();
                        Ok(Some(self.query(cx, &format!("senders/{sender}"))))
                    }
                    None => Ok(Some(self.node(cx, job))),
                }
            }
            Step::SenderResource => {
                let doc: Value = serde_json::from_slice(&response.body)
                    .map_err(|_| device_error(format!("{what} is not JSON")))?;
                let manifest = doc
                    .get("manifest_href")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|h| h.starts_with("http://") || h.starts_with("https://"))
                    .map(String::from);
                match manifest {
                    Some(url) => {
                        job.step = Step::Manifest;
                        Ok(Some(self.send(cx, "GET", url, None)))
                    }
                    None => Ok(Some(self.node(cx, job))),
                }
            }
            Step::Manifest => {
                let sdp = String::from_utf8_lossy(&response.body).into_owned();
                if let Action::Stage(body) = &mut job.action {
                    body["transport_file"] = json!({"data": sdp, "type": "application/sdp"});
                }
                Ok(Some(self.node(cx, job)))
            }
            Step::Node => {
                let value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
                cx.complete(job.command, Ok(Outcome::Value { value }));
                Ok(None)
            }
        }
    }

    /// The request to the Node's Connection API.
    fn node(&mut self, cx: &mut Cx, job: &mut Job) -> RequestId {
        job.step = Step::Node;
        let base = format!(
            "{}single/{}/{}",
            job.href.as_deref().unwrap_or_default(),
            job.side.plural(),
            job.id
        );
        match &job.action {
            Action::Stage(body) => {
                let body = body.clone();
                self.send(cx, "PATCH", format!("{base}/staged"), Some(body))
            }
            Action::Active => self.send(cx, "GET", format!("{base}/active"), None),
        }
    }

    /// Fail every command in flight (the device closing).
    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        for job in self.jobs.drain(..) {
            cx.complete(job.command, Err(CommandError::Closed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action as Act, CommandResult};

    const RX: &str = "3b8be755-08ff-452b-b217-c9151eb21193";
    const TX: &str = "171d5c80-7fb3-493d-a0e0-de5ba6d4a1a5";
    const DEV: &str = "58f6b536-ca4c-43fd-880a-9df2501fc125";

    fn params(v: Value) -> Params {
        serde_json::from_value(v).unwrap()
    }

    fn requests(actions: &[Act]) -> Vec<(RequestId, HttpRequest)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Act::Http { id, request } => Some((*id, request.clone())),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Act]) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Act::Complete { result, .. } => Some(result.clone()),
            _ => None,
        })
    }

    fn ok(body: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        })
    }

    fn connector() -> Connector {
        Connector::new(
            "http://10.0.0.9:8080".into(),
            "v1.3".into(),
            Some("tok".into()),
        )
    }

    /// Answer the one request the last step made, and return the next.
    fn answer(c: &mut Connector, last: &[Act], body: Result<HttpResponse, String>) -> Vec<Act> {
        let (id, _) = requests(last).pop().expect("a request");
        let mut cx = Cx::new(0);
        c.http_response(&mut cx, id, body);
        cx.take()
    }

    const DEVICE: &str = r#"{"id":"58f6b536-ca4c-43fd-880a-9df2501fc125","controls":[
        {"type":"urn:x-nmos:control:sr-ctrl/v1.0","href":"http://10.0.0.20/x-nmos/connection/v1.0/"},
        {"type":"urn:x-nmos:control:sr-ctrl/v1.1","href":"http://10.0.0.20/x-nmos/connection/v1.1"},
        {"type":"urn:x-manufacturer:control:generic","href":"http://10.0.0.20/x-manufacturer/control/"}]}"#;

    #[test]
    fn connect_receiver_finds_the_connection_api_and_the_senders_sdp() {
        let mut c = connector();
        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            7,
            "connect_receiver",
            &params(json!({"receiver_id": RX, "sender_id": TX})),
        );
        let a = cx.take();
        let r = requests(&a);
        assert_eq!(
            r[0].1.url,
            format!("http://10.0.0.9:8080/x-nmos/query/v1.3/receivers/{RX}")
        );
        assert!(r[0]
            .1
            .headers
            .contains(&("Authorization".into(), "Bearer tok".into())));
        let a = answer(
            &mut c,
            &a,
            ok(&format!(r#"{{"id":"{RX}","device_id":"{DEV}"}}"#)),
        );
        assert_eq!(
            requests(&a)[0].1.url,
            format!("http://10.0.0.9:8080/x-nmos/query/v1.3/devices/{DEV}")
        );
        let a = answer(&mut c, &a, ok(DEVICE));
        assert_eq!(
            requests(&a)[0].1.url,
            format!("http://10.0.0.9:8080/x-nmos/query/v1.3/senders/{TX}")
        );
        let a = answer(
            &mut c,
            &a,
            ok(
                r#"{"manifest_href":"http://10.0.0.30/x-nmos/connection/v1.1/single/senders/s/transportfile"}"#,
            ),
        );
        assert_eq!(
            requests(&a)[0].1.url,
            "http://10.0.0.30/x-nmos/connection/v1.1/single/senders/s/transportfile"
        );
        let a = answer(&mut c, &a, ok("v=0\r\n"));
        let (_, patch) = requests(&a).pop().unwrap();
        assert_eq!(patch.method, "PATCH");
        // The highest sr-ctrl version, a slash added.
        assert_eq!(
            patch.url,
            format!("http://10.0.0.20/x-nmos/connection/v1.1/single/receivers/{RX}/staged")
        );
        let body: Value = serde_json::from_slice(patch.body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"sender_id": TX, "master_enable": true, "activation": {"mode": "activate_immediate"},
                   "transport_file": {"data": "v=0\r\n", "type": "application/sdp"}})
        );
        let a = answer(&mut c, &a, ok(r#"{"sender_id":"x","master_enable":true}"#));
        assert_eq!(
            completed(&a),
            Some(Ok(Outcome::Value {
                value: json!({"sender_id": "x", "master_enable": true})
            }))
        );
    }

    #[test]
    fn a_sender_without_a_manifest_is_connected_by_id() {
        let mut c = connector();
        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            7,
            "connect_receiver",
            &params(json!({"receiver_id": RX, "sender_id": TX})),
        );
        let a = cx.take();
        let a = answer(&mut c, &a, ok(&format!(r#"{{"device_id":"{DEV}"}}"#)));
        let a = answer(&mut c, &a, ok(DEVICE));
        let a = answer(&mut c, &a, ok(r#"{"manifest_href":null}"#));
        let (_, patch) = requests(&a).pop().unwrap();
        let body: Value = serde_json::from_slice(patch.body.as_ref().unwrap()).unwrap();
        assert!(body.get("transport_file").is_none());
        assert_eq!(body["sender_id"], TX);
    }

    #[test]
    fn disconnect_and_reads_go_straight_to_the_node() {
        let mut c = connector();
        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            1,
            "disconnect_receiver",
            &params(json!({"receiver_id": RX})),
        );
        let a = cx.take();
        let a = answer(&mut c, &a, ok(&format!(r#"{{"device_id":"{DEV}"}}"#)));
        let a = answer(&mut c, &a, ok(DEVICE));
        let (_, patch) = requests(&a).pop().unwrap();
        let body: Value = serde_json::from_slice(patch.body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"sender_id": null, "master_enable": false, "activation": {"mode": "activate_immediate"}})
        );

        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            2,
            "get_sender_active",
            &params(json!({"sender_id": TX})),
        );
        let a = cx.take();
        assert!(requests(&a)[0].1.url.ends_with(&format!("/senders/{TX}")));
        let a = answer(&mut c, &a, ok(&format!(r#"{{"device_id":"{DEV}"}}"#)));
        let a = answer(&mut c, &a, ok(DEVICE));
        let (_, get) = requests(&a).pop().unwrap();
        assert_eq!(get.method, "GET");
        assert_eq!(
            get.url,
            format!("http://10.0.0.20/x-nmos/connection/v1.1/single/senders/{TX}/active")
        );
    }

    #[test]
    fn failures_name_where_they_happened() {
        let mut c = connector();
        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            3,
            "stage_receiver",
            &params(json!({"receiver_id": RX, "staged": {"master_enable": true}})),
        );
        let a = cx.take();
        let a = answer(&mut c, &a, ok(&format!(r#"{{"device_id":"{DEV}"}}"#)));
        // A device without a Connection API.
        let a = answer(&mut c, &a, ok(r#"{"controls":[]}"#));
        assert!(matches!(
            completed(&a),
            Some(Err(CommandError::DeviceError { .. }))
        ));

        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            4,
            "get_receiver_active",
            &params(json!({"receiver_id": RX})),
        );
        let a = cx.take();
        let a = answer(
            &mut c,
            &a,
            Ok(HttpResponse {
                status: 404,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            }),
        );
        assert!(matches!(
            completed(&a),
            Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "404"
        ));

        let mut cx = Cx::new(0);
        c.command(
            &mut cx,
            5,
            "stage_sender",
            &params(json!({"sender_id": TX, "staged": [1]})),
        );
        assert!(matches!(
            completed(&cx.take()),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }
}
