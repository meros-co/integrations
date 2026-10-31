//! Panasonic LUMIX cameras' Wi-Fi remote control: HTTP GET of `/cam.cgi` on
//! port 80, the protocol of Panasonic's Image App and LUMIX Sync. Panasonic
//! publishes no document; this follows the MIT-licensed Lumix Remote project
//! (verified on a DC-S5) and lx100-api (see the spec's sources).
//!
//! Connecting sends one `getstate` (any answer shows the camera is there),
//! reads the camera's UPnP description
//! (`:60606/Lumix/Server0/ddd`) for its name, model and `X_CamCapability`,
//! then asks for access with `accctrl`:
//!
//! - plain: `req_acc` with the controller's id and name, answered
//!   `ok_under_research...` while the user is asked on the camera (asked
//!   again every [`ACCESS_RETRY`]) and `ok,<name>,remote,...` once accepted;
//! - encrypted (a description whose `X_CamCapability` ends in `1`, or the
//!   `handshake` setting): `req_acc_g` answers `ok,<8 hex digits>`, a nonce,
//!   and `req_acc_e` carries the id and name transformed with it
//!   ([`encrypt`]); its accepting answer's fifth field is a session id sent
//!   as `X-SESSION_ID` on every later request. A camera refusing `req_acc_g`
//!   is asked the plain way when the setting is `auto`.
//!
//! Accepted, the module sends its name (`setsetting device_name`), then polls
//! `getstate` every [`POLL_EVERY`] (every second during live view), which
//! also keeps the camera from dropping it. Requests go one at a time, the
//! camera's web server being small; commands go ahead of polls. Mode
//! switches are kept [`SWITCH_GAP`] apart, since close ones wedge the S5.
//!
//! The `live` stream binds the `liveview_port` setting's UDP port, switches
//! the camera to record mode when it is not in it, and asks for frames with
//! `startstream`; each datagram is a header and one JPEG, published whole.
//! When the last watcher leaves, `stopstream`.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis,
    Module, OpenContext, Outcome, PollStamps, RequestId,
};

const PORT: u16 = 80;
const UPNP_PORT: u16 = 60606;
const FILE_PORT: u16 = 50001;
const DDD_PATH: &str = "/Lumix/Server0/ddd";
const CDS_PATH: &str = "/Server0/CDS_control";
const LIVE: Key = "live";
const LIVE_SOCKET: Key = "liveview";

const POLL: Key = "poll";
const ACCESS: Key = "access";
const RETRY: Key = "retry";

const REQUEST_TIMEOUT: Millis = 6_000;
const DOWNLOAD_TIMEOUT: Millis = 600_000;
const POLL_EVERY: Millis = 2_000;
const LIVE_POLL_EVERY: Millis = 1_000;
const ACCESS_RETRY: Millis = 1_000;
/// Mode switches closer than this wedge the S5 (Lumix Remote's logs).
const SWITCH_GAP: Millis = 10_000;
/// Unanswered polls in a row before the camera is taken as gone.
const POLL_STRIKES: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// LUMIX Sync's stand-in for a zero nonce.
const ZERO_NONCE: u32 = 0x9032_782c;
const MODE_SWITCHES: [&str; 3] = ["recmode", "playmode", "pictmode"];

/// The `req_acc_e` transform of a field: its UTF-8 bytes padded with 0xFF to
/// whole 4-byte blocks, each block read big-endian, XOR the nonce (a zero
/// nonce standing for [`ZERO_NONCE`]), byte-swapped and written as 8
/// lowercase hex digits.
pub(crate) fn encrypt(text: &str, nonce: u32) -> String {
    let n = if nonce == 0 { ZERO_NONCE } else { nonce };
    let mut raw = text.as_bytes().to_vec();
    while !raw.len().is_multiple_of(4) {
        raw.push(0xFF);
    }
    raw.chunks(4)
        .map(|b| {
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) ^ n;
            format!("{:08x}", v.swap_bytes())
        })
        .collect()
}

/// The JPEG in a live view datagram: from the first SOI to the last EOI.
fn jpeg_in(data: &[u8]) -> Option<&[u8]> {
    let start = data.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let end = data.windows(2).rposition(|w| w == [0xFF, 0xD9])?;
    (end > start).then(|| &data[start..end + 2])
}

/// Percent-encodes a query value, keeping the characters the camera expects
/// as they are (`2304/256`, `4:3`, `-1/3`).
fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"/:._-".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn query(mode: &str, kind: Option<&str>, value: Option<&str>, value2: Option<&str>) -> String {
    let mut q = format!("mode={}", encode(mode));
    for (key, v) in [("type", kind), ("value", value), ("value2", value2)] {
        if let Some(v) = v {
            q.push_str(&format!("&{key}={}", encode(v)));
        }
    }
    q
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Describe,
    Access,
    Ready,
    Down,
}

/// What a command's answer becomes.
#[derive(Debug, Clone, PartialEq)]
enum Answer {
    Ack,
    /// The getstate fields as an object.
    State,
    /// The body as text.
    Text,
    /// getsetting's `<settingvalue>` attributes as an object.
    Setting,
    /// get_content_info's fields as an object.
    Fields,
    /// A Browse answer's DIDL-Lite text.
    Browse,
    /// A saved file.
    File(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Probe,
    Describe,
    AccessNonce,
    Access,
    Name,
    Poll,
    StartStream,
    StopStream,
    LiveRecMode,
    Command(CommandId, Answer),
}

#[derive(Debug, Clone)]
struct Request {
    purpose: Purpose,
    method: &'static str,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    save: Option<PathBuf>,
    timeout: Millis,
}

pub(crate) struct Lumix {
    ip: IpAddr,
    host: String,
    port: u16,
    name: String,
    client_id: String,
    handshake: String,
    live_port: u16,
    phase: Phase,
    encrypted: bool,
    /// The access request being made, kept to send again while the user is
    /// asked on the camera.
    access: Option<(String, String, String)>,
    session: Option<String>,
    queue: VecDeque<Request>,
    in_flight: Option<(RequestId, Request)>,
    next_id: RequestId,
    stamps: PollStamps,
    strikes: u32,
    cammode: Option<String>,
    last_switch: Option<Millis>,
    watching: bool,
    live_on: bool,
    retry_after: Millis,
}

impl Lumix {
    pub(crate) fn new(ctx: OpenContext) -> Lumix {
        let setting = |name: &str| {
            ctx.settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let name = match setting("device_name") {
            n if n.is_empty() => "Meros".to_string(),
            n => n,
        };
        let client_id = match setting("client_id") {
            id if id.is_empty() => format!("meros-{}", name.replace(' ', "-")),
            id => id,
        };
        let handshake = match setting("handshake") {
            h if h.is_empty() => "auto".to_string(),
            h => h,
        };
        let live_port = ctx
            .settings
            .get("liveview_port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(49_199);
        let host = match ctx.host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        Lumix {
            ip: ctx.host,
            host,
            port: ctx.port.unwrap_or(PORT),
            name,
            client_id,
            handshake,
            live_port,
            phase: Phase::Describe,
            encrypted: false,
            access: None,
            session: None,
            queue: VecDeque::new(),
            in_flight: None,
            next_id: 1,
            stamps: PollStamps::default(),
            strikes: 0,
            cammode: None,
            last_switch: None,
            watching: false,
            live_on: false,
            retry_after: RETRY_MIN,
        }
    }

    fn cgi_url(&self, q: &str) -> String {
        format!("http://{}:{}/cam.cgi?{q}", self.host, self.port)
    }

    fn cgi(&self, purpose: Purpose, q: String) -> Request {
        let mut headers = Vec::new();
        if let Some(id) = &self.session {
            headers.push(("X-SESSION_ID".to_string(), id.clone()));
        }
        Request {
            purpose,
            method: "GET",
            url: self.cgi_url(&q),
            headers,
            body: None,
            save: None,
            timeout: REQUEST_TIMEOUT,
        }
    }

    /// Queues a request: commands ahead of everything but other commands.
    fn enqueue(&mut self, cx: &mut Cx, request: Request, urgent: bool) {
        if urgent {
            let at = self
                .queue
                .iter()
                .position(|r| !matches!(r.purpose, Purpose::Command(..)))
                .unwrap_or(self.queue.len());
            self.queue.insert(at, request);
        } else {
            self.queue.push_back(request);
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        let id = self.next_id;
        self.next_id += 1;
        let http = HttpRequest {
            method: request.method,
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
            body_stream: None,
            timeout: Some(request.timeout),
            accept_invalid_certs: false,
            digest: None,
        };
        match &request.save {
            Some(to) => cx.http_save(id, http, to.clone()),
            None => cx.http(id, http),
        }
        self.in_flight = Some((id, request));
    }

    /// A first getstate on cam.cgi: any HTTP answer, accepted or not, shows
    /// the camera is there; its description is read next.
    fn probe(&mut self, cx: &mut Cx) {
        self.phase = Phase::Describe;
        cx.connection(Connection::Connecting);
        let r = self.cgi(Purpose::Probe, query("getstate", None, None, None));
        self.enqueue(cx, r, false);
    }

    fn describe(&mut self, cx: &mut Cx) {
        let request = Request {
            purpose: Purpose::Describe,
            method: "GET",
            url: format!("http://{}:{UPNP_PORT}{DDD_PATH}", self.host),
            headers: Vec::new(),
            body: None,
            save: None,
            timeout: REQUEST_TIMEOUT,
        };
        self.enqueue(cx, request, false);
    }

    fn described(&mut self, cx: &mut Cx, body: &str) {
        let mut fields = Map::new();
        let mut capability = String::new();
        if let Ok(doc) =
            roxmltree::Document::parse(body.trim_start_matches('\u{feff}').trim_start())
        {
            for node in doc.descendants().filter(|n| n.is_element()) {
                let text = node.text().unwrap_or("").trim().to_string();
                let key = match node.tag_name().name() {
                    "friendlyName" => "friendly_name",
                    "modelNumber" => "model",
                    "serialNumber" => "serial",
                    "X_FirmVersion" => "firmware",
                    "UDN" => "udn",
                    "X_CamCapability" => {
                        capability = text;
                        continue;
                    }
                    _ => continue,
                };
                fields.insert(key.into(), json!(text));
            }
        }
        cx.state(json!({"device": fields, "access": "requesting"}));
        self.encrypted = match self.handshake.as_str() {
            "plain" => false,
            "encrypted" => true,
            _ => capability.ends_with('1'),
        };
        self.phase = Phase::Access;
        if self.encrypted {
            let q = query("accctrl", Some("req_acc_g"), None, None);
            let r = self.cgi(Purpose::AccessNonce, q);
            self.enqueue(cx, r, false);
        } else {
            self.request_access(cx, "req_acc", self.client_id.clone(), self.name.clone());
        }
    }

    fn request_access(&mut self, cx: &mut Cx, kind: &str, v1: String, v2: String) {
        self.access = Some((kind.to_string(), v1.clone(), v2.clone()));
        let q = query("accctrl", Some(kind), Some(&v1), Some(&v2));
        let r = self.cgi(Purpose::Access, q);
        self.enqueue(cx, r, false);
    }

    fn accepted(&mut self, cx: &mut Cx, fields: &[&str]) {
        if self.encrypted {
            self.session = fields
                .get(4)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
        }
        self.access = None;
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        self.strikes = 0;
        cx.connection(Connection::Connected);
        cx.state(json!({"access": "accepted", "encrypted": self.encrypted}));
        let q = query("setsetting", Some("device_name"), Some(&self.name), None);
        let name = self.cgi(Purpose::Name, q);
        self.enqueue(cx, name, false);
        self.poll(cx);
        if self.watching {
            self.start_live(cx);
        }
    }

    fn poll(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready {
            return;
        }
        if !self
            .queue
            .iter()
            .chain(self.in_flight.iter().map(|(_, r)| r))
            .any(|r| r.purpose == Purpose::Poll)
        {
            let r = self.cgi(Purpose::Poll, query("getstate", None, None, None));
            self.enqueue(cx, r, false);
        }
        cx.set_timer(
            POLL,
            if self.live_on {
                LIVE_POLL_EVERY
            } else {
                POLL_EVERY
            },
        );
    }

    fn start_live(&mut self, cx: &mut Cx) {
        cx.udp_open(
            LIVE_SOCKET,
            Bind::Any {
                address: None,
                port: self.live_port,
            },
        );
        if self.phase != Phase::Ready {
            return;
        }
        if self.cammode.as_deref() != Some("rec") && self.switch_allowed(cx) {
            let r = self.cgi(
                Purpose::LiveRecMode,
                query("camcmd", None, Some("recmode"), None),
            );
            self.enqueue(cx, r, true);
        }
        let port = self.live_port.to_string();
        let r = self.cgi(
            Purpose::StartStream,
            query("startstream", None, Some(&port), None),
        );
        self.enqueue(cx, r, true);
    }

    fn stop_live(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Ready && self.live_on {
            let r = self.cgi(Purpose::StopStream, query("stopstream", None, None, None));
            self.enqueue(cx, r, true);
        }
        self.live_on = false;
        cx.udp_close(LIVE_SOCKET);
        cx.state(json!({"liveview": false}));
    }

    /// Whether a mode switch may go now; notes it when it may.
    fn switch_allowed(&mut self, cx: &mut Cx) -> bool {
        let now = cx.now();
        if self
            .last_switch
            .is_some_and(|at| now.saturating_sub(at) < SWITCH_GAP)
        {
            return false;
        }
        self.last_switch = Some(now);
        true
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if let Some((_, r)) = self.in_flight.take() {
            if let Purpose::Command(id, _) = r.purpose {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for r in self.queue.drain(..) {
            if let Purpose::Command(id, _) = r.purpose {
                cx.complete(id, Err(CommandError::NotConnected));
            }
        }
        for key in [POLL, ACCESS] {
            cx.cancel_timer(key);
        }
        self.phase = Phase::Down;
        self.session = None;
        self.access = None;
        self.cammode = None;
        if self.live_on {
            self.live_on = false;
            cx.state(json!({"liveview": false}));
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The request a command makes, or why it cannot be made.
    fn command_request(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        params: &Params,
    ) -> Result<Request, CommandError> {
        let s = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| CommandError::InvalidParams {
                    message: format!("'{k}' is required"),
                })
        };
        let n = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_i64)
                .ok_or_else(|| CommandError::InvalidParams {
                    message: format!("'{k}' is required"),
                })
        };
        let ack = |q: String| (q, Answer::Ack);
        let camcmd = |v: &str| ack(query("camcmd", None, Some(v), None));
        let set = |t: &str, v: &str| ack(query("setsetting", Some(t), Some(v), None));
        let (q, answer) = match name {
            "get_state" => (query("getstate", None, None, None), Answer::State),
            "get_info" => (
                query("getinfo", Some(&s("type")?), None, None),
                Answer::Text,
            ),
            "get_lens" => (query("getinfo", Some("lens"), None, None), Answer::Text),
            "get_setting" => (
                query("getsetting", Some(&s("type")?), None, None),
                Answer::Setting,
            ),
            "set_setting" => set(&s("type")?, &s("value")?),
            "set_setting_pair" => ack(query(
                "setsetting",
                Some(&s("type")?),
                Some(&s("value")?),
                Some(&s("value2")?),
            )),
            "camcmd" => camcmd(&s("value")?),
            "camctrl" => ack(query(
                "camctrl",
                Some(&s("type")?),
                Some(&s("value")?),
                None,
            )),
            "camctrl_pair" => ack(query(
                "camctrl",
                Some(&s("type")?),
                Some(&s("value")?),
                Some(&s("value2")?),
            )),
            "capture" => camcmd("capture"),
            "cancel_capture" => camcmd("capture_cancel"),
            "start_recording" => camcmd("video_recstart"),
            "stop_recording" => camcmd("video_recstop"),
            "record_mode" => camcmd("recmode"),
            "playback_mode" => camcmd("playmode"),
            "power_off" => camcmd("poweroff"),
            "one_shot_af" => camcmd("oneshot_af"),
            "touch_af" => ack(query(
                "camctrl",
                Some("touch"),
                Some(&format!("{}/{}", n("x")?, n("y")?)),
                Some("on"),
            )),
            "touch_release" => camcmd("touchrelease"),
            "zoom" => match s("action")?.as_str() {
                "stop" => camcmd("zoomstop"),
                step => camcmd(step),
            },
            "focus" => ack(query("camctrl", Some("focus"), Some(&s("step")?), None)),
            "set_iso" => set("iso", &s("value")?),
            "set_aperture" => set("focal", &s("value")?),
            "set_shutter_speed" => set("shtrspeed", &s("value")?),
            "set_exposure_compensation" => set("exposure", &s("value")?),
            "set_focus_mode" => set("focusmode", &s("mode")?),
            "set_af_mode" => set("afmode", &s("mode")?),
            "set_liveview_size" => set("liveviewsize", &s("size")?),
            "get_content_info" => (query("get_content_info", None, None, None), Answer::Fields),
            "browse_content" => return self.browse(id, params),
            "download_file" => return self.download(id, params),
            _ => {
                return Err(CommandError::UnknownCommand {
                    command: name.into(),
                })
            }
        };
        // A mode switch too soon after the last one wedges the camera.
        let switch = MODE_SWITCHES
            .iter()
            .any(|m| q == query("camcmd", None, Some(m), None));
        if switch && !self.switch_allowed(cx) {
            return Err(CommandError::DeviceError {
                code: None,
                message: format!(
                    "a mode switch within {} s of the last one: the camera can wedge",
                    SWITCH_GAP / 1000
                ),
            });
        }
        Ok(self.cgi(Purpose::Command(id, answer), q))
    }

    /// A UPnP ContentDirectory Browse of the card, in playback mode.
    fn browse(&self, id: CommandId, params: &Params) -> Result<Request, CommandError> {
        let object = params
            .get("object_id")
            .and_then(Value::as_str)
            .unwrap_or("0");
        let start = params.get("start").and_then(Value::as_u64).unwrap_or(0);
        let count = params.get("count").and_then(Value::as_u64).unwrap_or(50);
        let escaped = object
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:Browse xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\" xmlns:pana=\"urn:schemas-panasonic-com:pana\">\
<ObjectID>{escaped}</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag><Filter>*</Filter>\
<StartingIndex>{start}</StartingIndex><RequestedCount>{count}</RequestedCount><SortCriteria></SortCriteria>\
<pana:X_FromCP>LumixLink2.0</pana:X_FromCP></u:Browse></s:Body></s:Envelope>"
        );
        Ok(Request {
            purpose: Purpose::Command(id, Answer::Browse),
            method: "POST",
            url: format!("http://{}:{UPNP_PORT}{CDS_PATH}", self.host),
            headers: vec![
                (
                    "SOAPACTION".into(),
                    "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"".into(),
                ),
                ("Content-Type".into(), "text/xml; charset=\"utf-8\"".into()),
            ],
            body: Some(body.into_bytes()),
            save: None,
            timeout: REQUEST_TIMEOUT,
        })
    }

    /// A file on the card, saved to a host file.
    fn download(&self, id: CommandId, params: &Params) -> Result<Request, CommandError> {
        let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
            CommandError::InvalidParams {
                message: "'name' is required".into(),
            }
        })?;
        let to = params
            .get("to")
            .and_then(|v| v.get("file"))
            .and_then(Value::as_str)
            .ok_or_else(|| CommandError::InvalidParams {
                message: "'to' must be {\"file\": <path>}".into(),
            })?;
        Ok(Request {
            purpose: Purpose::Command(id, Answer::File(to.to_string())),
            method: "GET",
            url: format!("http://{}:{FILE_PORT}/{}", self.host, encode(name)),
            headers: Vec::new(),
            body: None,
            save: Some(PathBuf::from(to)),
            timeout: DOWNLOAD_TIMEOUT,
        })
    }

    fn answered(&mut self, cx: &mut Cx, request: Request, response: HttpResponse) {
        let body = String::from_utf8_lossy(&response.body).trim().to_string();
        let ok_status = (200..300).contains(&response.status);
        match request.purpose {
            Purpose::Probe => self.describe(cx),
            Purpose::Describe => {
                if ok_status {
                    self.described(cx, &body);
                } else {
                    self.lost(
                        cx,
                        format!("the camera's description answered HTTP {}", response.status),
                    );
                }
            }
            Purpose::AccessNonce => {
                let fields: Vec<&str> = body.split(',').collect();
                let nonce = (fields.first() == Some(&"ok"))
                    .then(|| fields.get(1))
                    .flatten()
                    .and_then(|h| u32::from_str_radix(h.trim(), 16).ok());
                match nonce {
                    Some(nonce) => {
                        let v1 = encrypt(&self.client_id, nonce);
                        let v2 = encrypt(&self.name, nonce);
                        self.request_access(cx, "req_acc_e", v1, v2);
                    }
                    None if self.handshake == "auto" => {
                        cx.log(
                            Level::Info,
                            format!("req_acc_g answered {body}: asking the plain way"),
                        );
                        self.encrypted = false;
                        self.request_access(
                            cx,
                            "req_acc",
                            self.client_id.clone(),
                            self.name.clone(),
                        );
                    }
                    None => self.lost(cx, format!("the camera refused req_acc_g: {body}")),
                }
            }
            Purpose::Access => {
                let fields: Vec<&str> = body.split(',').collect();
                if fields.first() == Some(&"ok") {
                    self.accepted(cx, &fields);
                } else if body.starts_with("ok_under_research") {
                    cx.state(json!({"access": "waiting"}));
                    cx.connection(Connection::Connecting);
                    cx.set_timer(ACCESS, ACCESS_RETRY);
                } else {
                    cx.state(json!({"access": "refused"}));
                    self.lost(cx, format!("the camera refused access: {body}"));
                }
            }
            Purpose::Name | Purpose::LiveRecMode => {}
            Purpose::StartStream => {
                if result(&body).as_deref() == Some("ok") {
                    self.live_on = true;
                    cx.state(json!({"liveview": true}));
                    cx.set_timer(POLL, LIVE_POLL_EVERY);
                } else {
                    cx.log(Level::Warning, format!("startstream answered {body}"));
                }
            }
            Purpose::StopStream => {}
            Purpose::Poll => match result(&body).as_deref() {
                Some("ok") => {
                    self.strikes = 0;
                    self.stamps.answered(cx, "status");
                    let fields = state_fields(&body);
                    if let Some(mode) = fields.get("cammode").and_then(Value::as_str) {
                        self.cammode = Some(mode.to_string());
                    }
                    cx.state(json!({ "status": fields }));
                }
                Some("err_reject") => self.lost(cx, "the camera dropped this controller".into()),
                _ => self.missed_poll(cx),
            },
            Purpose::Command(id, answer) => {
                cx.complete(id, outcome(&answer, &response, &body));
            }
        }
    }

    fn missed_poll(&mut self, cx: &mut Cx) {
        self.strikes += 1;
        if self.strikes >= POLL_STRIKES {
            self.lost(
                cx,
                format!("{POLL_STRIKES} state polls in a row unanswered"),
            );
        }
    }
}

/// `<result>` of a `<camrply>` answer, or the first field of a CSV one.
fn result(body: &str) -> Option<String> {
    if body.starts_with('<') {
        let doc = roxmltree::Document::parse(body).ok()?;
        let r = doc
            .descendants()
            .find(|n| n.has_tag_name("result"))?
            .text()
            .unwrap_or("")
            .trim()
            .to_string();
        Some(r)
    } else {
        body.split(',').next().map(|s| s.trim().to_string())
    }
}

/// The children of getstate's `<state>`, by tag.
fn state_fields(body: &str) -> Map<String, Value> {
    let mut out = Map::new();
    if let Ok(doc) = roxmltree::Document::parse(body) {
        if let Some(state) = doc.descendants().find(|n| n.has_tag_name("state")) {
            for child in state.children().filter(|n| n.is_element()) {
                out.insert(
                    child.tag_name().name().to_string(),
                    json!(child.text().unwrap_or("").trim()),
                );
            }
        }
    }
    out
}

/// What a command's answer comes to.
fn outcome(answer: &Answer, response: &HttpResponse, body: &str) -> Result<Outcome, CommandError> {
    if !(200..300).contains(&response.status) {
        return Err(CommandError::DeviceError {
            code: Some(response.status.to_string()),
            message: format!("HTTP {}", response.status),
        });
    }
    match answer {
        Answer::File(path) => {
            let size = response
                .header_values("content-length")
                .next()
                .and_then(|v| v.trim().parse::<u64>().ok());
            let content_type = response.header_values("content-type").next();
            return Ok(Outcome::Value {
                value: json!({"file": path, "size": size, "content_type": content_type}),
            });
        }
        Answer::Browse => {
            let doc = roxmltree::Document::parse(body).map_err(|e| CommandError::DeviceError {
                code: None,
                message: format!("not a Browse answer: {e}"),
            })?;
            let text = |tag: &str| {
                doc.descendants()
                    .find(|n| n.has_tag_name(tag))
                    .and_then(|n| n.text())
                    .map(str::to_string)
            };
            let number = |tag: &str| text(tag).and_then(|t| t.trim().parse::<u64>().ok());
            return Ok(Outcome::Value {
                value: json!({
                    "result": text("Result").unwrap_or_default(),
                    "returned": number("NumberReturned"),
                    "total": number("TotalMatches"),
                }),
            });
        }
        _ => {}
    }
    let r = result(body).unwrap_or_default();
    if r != "ok" {
        return Err(CommandError::DeviceError {
            code: (!r.is_empty()).then(|| r.clone()),
            message: format!(
                "the camera answered {}",
                if r.is_empty() { body } else { &r }
            ),
        });
    }
    Ok(match answer {
        Answer::Ack => Outcome::Ack,
        Answer::State => Outcome::Value {
            value: Value::Object(state_fields(body)),
        },
        Answer::Text => Outcome::Value { value: json!(body) },
        Answer::Setting => {
            let mut out = Map::new();
            if let Ok(doc) = roxmltree::Document::parse(body) {
                if let Some(v) = doc.descendants().find(|n| n.has_tag_name("settingvalue")) {
                    for a in v.attributes() {
                        out.insert(a.name().to_string(), json!(a.value()));
                    }
                }
            }
            Outcome::Value {
                value: Value::Object(out),
            }
        }
        Answer::Fields => {
            let mut out = Map::new();
            if let Ok(doc) = roxmltree::Document::parse(body) {
                if let Some(root) = doc.descendants().find(|n| n.has_tag_name("camrply")) {
                    for child in root.children().filter(|n| n.is_element()) {
                        if child.tag_name().name() != "result" {
                            out.insert(
                                child.tag_name().name().to_string(),
                                json!(child.text().unwrap_or("").trim()),
                            );
                        }
                    }
                }
            }
            Outcome::Value {
                value: Value::Object(out),
            }
        }
        Answer::Browse | Answer::File(_) => unreachable!("answered above"),
    })
}

impl Module for Lumix {
    fn start(&mut self, cx: &mut Cx) {
        self.probe(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.command_request(cx, id, name, params) {
            Ok(request) => self.enqueue(cx, request, true),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some((current, request)) = self.in_flight.take() else {
            return;
        };
        if current != id {
            self.in_flight = Some((current, request));
            return;
        }
        match result {
            Ok(response) => {
                cx.alive();
                self.answered(cx, request, response);
            }
            Err(reason) => match request.purpose {
                Purpose::Command(cid, _) => {
                    cx.complete(cid, Err(CommandError::Transport { message: reason }))
                }
                Purpose::Poll => self.missed_poll(cx),
                Purpose::Name
                | Purpose::StopStream
                | Purpose::LiveRecMode
                | Purpose::StartStream => {}
                Purpose::Probe | Purpose::Describe | Purpose::AccessNonce | Purpose::Access => {
                    self.lost(cx, format!("the camera did not answer: {reason}"))
                }
            },
        }
        self.pump(cx);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        if socket != LIVE_SOCKET || from.ip() != self.ip || !self.watching {
            return;
        }
        if let Some(jpeg) = jpeg_in(data) {
            cx.frame(LIVE, "jpeg", jpeg.to_vec());
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if socket == LIVE_SOCKET {
            cx.log(
                Level::Warning,
                format!("live view port {}: {message}", self.live_port),
            );
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        if stream != LIVE || watching == self.watching {
            return;
        }
        self.watching = watching;
        if watching {
            self.start_live(cx);
        } else {
            self.stop_live(cx);
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY if self.phase == Phase::Down => self.probe(cx),
            ACCESS if self.phase == Phase::Access => {
                if let Some((kind, v1, v2)) = self.access.clone() {
                    self.request_access(cx, &kind, v1, v2);
                }
            }
            POLL if self.phase == Phase::Ready => self.poll(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.watching {
            cx.udp_close(LIVE_SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::Ipv4Addr;

    const CAMERA: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 54, 1));

    const DDD_PLAIN: &str = r#"<?xml version="1.0"?><root xmlns="urn:schemas-upnp-org:device-1-0" xmlns:pana="urn:schemas-panasonic-com:pana"><device><friendlyName>S5-123456</friendlyName><modelNumber>DC-S5</modelNumber><serialNumber>XX1234</serialNumber><UDN>uuid:4D454930-0100-1000-8000-0000AAAA</UDN><pana:X_FirmVersion>2.03</pana:X_FirmVersion><pana:X_CamCapability>0000</pana:X_CamCapability></device></root>"#;
    const DDD_ENCRYPTED: &str = r#"<root><device><friendlyName>S5M2-1</friendlyName><modelNumber>DC-S5M2</modelNumber><X_CamCapability>0001</X_CamCapability></device></root>"#;
    const STATE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><camrply><result>ok</result><state><batt>3/3</batt><cammode>play</cammode><sdcardstatus>write_enable</sdcardstatus><rec>off</rec></state></camrply>";
    const OK: &str =
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><camrply><result>ok</result></camrply>";

    fn context(settings: Value) -> OpenContext {
        OpenContext {
            host: CAMERA,
            host_name: None,
            port: None,
            model: "lumix".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        }
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } | Action::HttpSave { id, request, .. } => {
                    Some((*id, request.clone()))
                }
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

    fn reply(m: &mut Lumix, now: Millis, id: RequestId, body: &str) -> Vec<Action> {
        let mut cx = Cx::at(now, 1_793_000_000_000 + now);
        m.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: body.as_bytes().to_vec(),
            }),
        );
        cx.take()
    }

    /// Answers the request in flight and returns what followed.
    fn answer(m: &mut Lumix, now: Millis, body: &str) -> Vec<Action> {
        let id = m.in_flight.as_ref().expect("a request in flight").0;
        reply(m, now, id, body)
    }

    fn in_flight_url(m: &Lumix) -> String {
        m.in_flight.as_ref().unwrap().1.url.clone()
    }

    /// Described, accepted the plain way; the name and first poll answered.
    fn ready() -> Lumix {
        let mut m = Lumix::new(context(json!({"device_name": "Booth"})));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        answer(&mut m, 5, STATE);
        answer(&mut m, 10, DDD_PLAIN);
        answer(&mut m, 20, "ok,S5-123456,remote,encrypted");
        answer(&mut m, 30, OK);
        answer(&mut m, 40, STATE);
        m
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn the_transform_matches_lumix_syncs() {
        // Worked out independently, by the steps the spec's source gives.
        assert_eq!(
            encrypt("meros-Meros", 0x1a2b_3c4d),
            "224e4e7728710669b24f4468"
        );
        assert_eq!(encrypt("Meros", 0x1a2b_3c4d), "224e4e57b2c3d469");
        assert_eq!(encrypt("", 0), "");
        assert_eq!(encrypt("abcd", 0), encrypt("abcd", ZERO_NONCE));
    }

    #[test]
    fn a_plain_camera_is_described_asked_and_polled() {
        let mut m = Lumix::new(context(json!({"device_name": "Booth"})));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert_eq!(
            requests(&a)[0].1.url,
            "http://192.168.54.1:80/cam.cgi?mode=getstate"
        );
        // Any answer shows the camera is there, refused or not.
        answer(&mut m, 5, "<camrply><result>err_reject</result></camrply>");
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:60606/Lumix/Server0/ddd"
        );
        let a = answer(&mut m, 10, DDD_PLAIN);
        assert_eq!(
            state(&a)["device"],
            json!({"friendly_name": "S5-123456", "model": "DC-S5", "serial": "XX1234",
                   "udn": "uuid:4D454930-0100-1000-8000-0000AAAA", "firmware": "2.03"})
        );
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=accctrl&type=req_acc&value=meros-Booth&value2=Booth"
        );
        // Asked on the camera: waiting, asked again a second later.
        let a = answer(&mut m, 20, "ok_under_research_no_msg");
        assert_eq!(state(&a)["access"], "waiting");
        assert!(a.contains(&Action::SetTimer {
            key: ACCESS,
            after: ACCESS_RETRY
        }));
        let mut cx = Cx::new(1_020);
        m.timer(&mut cx, ACCESS);
        assert!(in_flight_url(&m).contains("type=req_acc&"));
        let a = answer(&mut m, 1_030, "ok,S5-123456,remote,encrypted");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&a)["access"], "accepted");
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=setsetting&type=device_name&value=Booth"
        );
        answer(&mut m, 1_040, OK);
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=getstate"
        );
        let a = answer(&mut m, 1_050, STATE);
        assert_eq!(state(&a)["status"]["cammode"], "play");
        assert_eq!(state(&a)["status"]["batt"], "3/3");
        assert!(state(&a)["polled"]["status"].is_number());
        // No session id on a plain camera.
        assert!(m.session.is_none());
    }

    #[test]
    fn an_encrypted_camera_gets_the_transformed_request_and_its_session_id() {
        let mut m = Lumix::new(context(
            json!({"device_name": "Meros", "client_id": "meros-Meros"}),
        ));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        answer(&mut m, 5, STATE);
        answer(&mut m, 10, DDD_ENCRYPTED);
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=accctrl&type=req_acc_g"
        );
        answer(&mut m, 20, "ok,1a2b3c4d");
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=accctrl&type=req_acc_e&value=224e4e7728710669b24f4468&value2=224e4e57b2c3d469"
        );
        let a = answer(&mut m, 30, "ok,S5M2-1,remote,open,SESSION42");
        assert_eq!(state(&a)["encrypted"], true);
        let headers = &m.in_flight.as_ref().unwrap().1.headers;
        assert!(headers.contains(&("X-SESSION_ID".into(), "SESSION42".into())));
    }

    #[test]
    fn a_refused_nonce_falls_back_to_the_plain_request() {
        let mut m = Lumix::new(context(json!({})));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        answer(&mut m, 5, STATE);
        answer(&mut m, 10, DDD_ENCRYPTED);
        answer(&mut m, 20, "err_param");
        assert_eq!(
            in_flight_url(&m),
            "http://192.168.54.1:80/cam.cgi?mode=accctrl&type=req_acc&value=meros-Meros&value2=Meros"
        );
    }

    #[test]
    fn a_refusal_is_retried_with_backoff() {
        let mut m = Lumix::new(context(json!({})));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        answer(&mut m, 5, STATE);
        answer(&mut m, 10, DDD_PLAIN);
        let a = answer(&mut m, 20, "err_reject");
        assert_eq!(state(&a)["access"], "refused");
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(1_100);
        m.timer(&mut cx, RETRY);
        assert!(in_flight_url(&m).ends_with("/cam.cgi?mode=getstate"));
    }

    #[test]
    fn each_command_sends_its_request() {
        let mut m = ready();
        for (name, input, q) in [
            ("get_state", json!({}), "mode=getstate"),
            (
                "get_info",
                json!({"type": "allmenu"}),
                "mode=getinfo&type=allmenu",
            ),
            ("get_lens", json!({}), "mode=getinfo&type=lens"),
            (
                "get_setting",
                json!({"type": "focusmode"}),
                "mode=getsetting&type=focusmode",
            ),
            (
                "set_setting",
                json!({"type": "peaking", "value": "high"}),
                "mode=setsetting&type=peaking&value=high",
            ),
            (
                "set_setting_pair",
                json!({"type": "a", "value": "1", "value2": "2"}),
                "mode=setsetting&type=a&value=1&value2=2",
            ),
            (
                "camcmd",
                json!({"value": "lcd_on"}),
                "mode=camcmd&value=lcd_on",
            ),
            (
                "camctrl",
                json!({"type": "mf_asst", "value": "500/500"}),
                "mode=camctrl&type=mf_asst&value=500/500",
            ),
            (
                "camctrl_pair",
                json!({"type": "touch", "value": "1/2", "value2": "off"}),
                "mode=camctrl&type=touch&value=1/2&value2=off",
            ),
            ("capture", json!({}), "mode=camcmd&value=capture"),
            (
                "cancel_capture",
                json!({}),
                "mode=camcmd&value=capture_cancel",
            ),
            (
                "start_recording",
                json!({}),
                "mode=camcmd&value=video_recstart",
            ),
            (
                "stop_recording",
                json!({}),
                "mode=camcmd&value=video_recstop",
            ),
            ("power_off", json!({}), "mode=camcmd&value=poweroff"),
            ("one_shot_af", json!({}), "mode=camcmd&value=oneshot_af"),
            (
                "touch_af",
                json!({"x": 840, "y": 234}),
                "mode=camctrl&type=touch&value=840/234&value2=on",
            ),
            ("touch_release", json!({}), "mode=camcmd&value=touchrelease"),
            (
                "zoom",
                json!({"action": "tele-fast"}),
                "mode=camcmd&value=tele-fast",
            ),
            (
                "zoom",
                json!({"action": "stop"}),
                "mode=camcmd&value=zoomstop",
            ),
            (
                "focus",
                json!({"step": "wide-normal"}),
                "mode=camctrl&type=focus&value=wide-normal",
            ),
            (
                "set_iso",
                json!({"value": "400"}),
                "mode=setsetting&type=iso&value=400",
            ),
            (
                "set_aperture",
                json!({"value": "1024/256"}),
                "mode=setsetting&type=focal&value=1024/256",
            ),
            (
                "set_shutter_speed",
                json!({"value": "3584/256"}),
                "mode=setsetting&type=shtrspeed&value=3584/256",
            ),
            (
                "set_exposure_compensation",
                json!({"value": "-1/3"}),
                "mode=setsetting&type=exposure&value=-1/3",
            ),
            (
                "set_focus_mode",
                json!({"mode": "mf"}),
                "mode=setsetting&type=focusmode&value=mf",
            ),
            (
                "set_af_mode",
                json!({"mode": "facedetection"}),
                "mode=setsetting&type=afmode&value=facedetection",
            ),
            (
                "set_liveview_size",
                json!({"size": "qvga"}),
                "mode=setsetting&type=liveviewsize&value=qvga",
            ),
            ("get_content_info", json!({}), "mode=get_content_info"),
        ] {
            let mut cx = Cx::new(5_000);
            m.command(&mut cx, 9, name, &params(input));
            assert_eq!(
                in_flight_url(&m),
                format!("http://192.168.54.1:80/cam.cgi?{q}"),
                "{name}"
            );
            let a = answer(&mut m, 5_010, OK);
            assert!(
                a.iter().any(|a| matches!(
                    a,
                    Action::Complete {
                        id: 9,
                        result: Ok(_)
                    }
                )),
                "{name}"
            );
        }
    }

    #[test]
    fn answers_become_values_and_errors() {
        let mut m = ready();
        let mut cx = Cx::new(5_000);
        m.command(
            &mut cx,
            1,
            "get_setting",
            &params(json!({"type": "focusmode"})),
        );
        let a = answer(
            &mut m,
            5_010,
            "<camrply><result>ok</result><settingvalue focusmode=\"afs\"></settingvalue></camrply>",
        );
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"focusmode": "afs"})
            })
        }));
        let mut cx = Cx::new(5_100);
        m.command(&mut cx, 2, "capture", &Params::new());
        let a = answer(
            &mut m,
            5_110,
            "<camrply><result>err_busy</result></camrply>",
        );
        assert!(a.iter().any(|a| matches!(a, Action::Complete {
            id: 2,
            result: Err(CommandError::DeviceError { code: Some(c), .. })
        } if c == "err_busy")));
        let mut cx = Cx::new(5_200);
        m.command(&mut cx, 3, "get_lens", &Params::new());
        let a = answer(&mut m, 5_210, "ok,2304/256,925/256,3072/256");
        assert!(a.contains(&Action::Complete {
            id: 3,
            result: Ok(Outcome::Value {
                value: json!("ok,2304/256,925/256,3072/256")
            })
        }));
        let mut cx = Cx::new(5_300);
        m.command(&mut cx, 4, "get_content_info", &Params::new());
        let a = answer(
            &mut m,
            5_310,
            "<camrply><result>ok</result><current_position>3</current_position><total_content_number>12</total_content_number></camrply>",
        );
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Value {
                value: json!({"current_position": "3", "total_content_number": "12"})
            })
        }));
    }

    #[test]
    fn mode_switches_are_kept_apart() {
        let mut m = ready();
        let mut cx = Cx::new(20_000);
        m.command(&mut cx, 1, "record_mode", &Params::new());
        assert!(in_flight_url(&m).ends_with("mode=camcmd&value=recmode"));
        answer(&mut m, 20_010, OK);
        let mut cx = Cx::new(25_000);
        m.command(&mut cx, 2, "playback_mode", &Params::new());
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                id: 2,
                result: Err(CommandError::DeviceError { .. })
            }]
        ));
        let mut cx = Cx::new(30_100);
        m.command(&mut cx, 3, "camcmd", &params(json!({"value": "playmode"})));
        assert!(in_flight_url(&m).ends_with("mode=camcmd&value=playmode"));
    }

    #[test]
    fn browse_and_download_reach_the_card() {
        let mut m = ready();
        let mut cx = Cx::new(5_000);
        m.command(
            &mut cx,
            1,
            "browse_content",
            &params(json!({"object_id": "0", "start": 0, "count": 10})),
        );
        let r = &m.in_flight.as_ref().unwrap().1;
        assert_eq!(r.method, "POST");
        assert_eq!(r.url, "http://192.168.54.1:60606/Server0/CDS_control");
        let body = String::from_utf8(r.body.clone().unwrap()).unwrap();
        assert!(body.contains("<ObjectID>0</ObjectID>"));
        assert!(body.contains("<RequestedCount>10</RequestedCount>"));
        let a = answer(
            &mut m,
            5_010,
            "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><u:BrowseResponse xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\"><Result>&lt;DIDL-Lite/&gt;</Result><NumberReturned>0</NumberReturned><TotalMatches>0</TotalMatches></u:BrowseResponse></s:Body></s:Envelope>",
        );
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"result": "<DIDL-Lite/>", "returned": 0, "total": 0})
            })
        }));
        let mut cx = Cx::new(6_000);
        m.command(
            &mut cx,
            2,
            "download_file",
            &params(json!({"name": "DO01111793.RW2", "to": {"file": "out.rw2"}})),
        );
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a, Action::HttpSave { request, to, .. }
            if request.url == "http://192.168.54.1:50001/DO01111793.RW2" && to == &PathBuf::from("out.rw2"))));
    }

    #[test]
    fn live_view_binds_starts_publishes_and_stops() {
        let mut m = ready();
        let mut cx = Cx::new(30_000);
        m.stream_watch(&mut cx, LIVE, true);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: LIVE_SOCKET,
            bind: Bind::Any {
                address: None,
                port: 49_199
            }
        }));
        // The camera is in play mode: record mode first.
        assert!(in_flight_url(&m).ends_with("mode=camcmd&value=recmode"));
        answer(&mut m, 30_010, OK);
        assert!(in_flight_url(&m).ends_with("mode=startstream&value=49199"));
        let a = answer(&mut m, 30_020, OK);
        assert_eq!(state(&a)["liveview"], true);
        let mut cx = Cx::new(30_030);
        let datagram = [&[0u8; 40][..], &[0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9]].concat();
        m.datagram(
            &mut cx,
            LIVE_SOCKET,
            SocketAddr::new(CAMERA, 5000),
            &datagram,
        );
        assert!(cx.take().contains(&Action::Frame {
            stream: LIVE,
            format: "jpeg",
            data: vec![0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9]
        }));
        // From anyone else, nothing.
        let mut cx = Cx::new(30_040);
        m.datagram(
            &mut cx,
            LIVE_SOCKET,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 5000),
            &datagram,
        );
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(31_000);
        m.stream_watch(&mut cx, LIVE, false);
        let a = cx.take();
        assert!(a.contains(&Action::UdpClose {
            socket: LIVE_SOCKET
        }));
        assert!(in_flight_url(&m).ends_with("mode=stopstream"));
    }

    #[test]
    fn unanswered_polls_and_a_dropped_session_reconnect() {
        let mut m = ready();
        for t in [3_000, 5_000] {
            let mut cx = Cx::new(t);
            m.timer(&mut cx, POLL);
            let id = m.in_flight.as_ref().unwrap().0;
            let mut cx = Cx::new(t + 6_000);
            m.http_response(&mut cx, id, Err("timed out".into()));
            assert!(!cx.take().iter().any(|a| matches!(a, Action::Connection(_))));
        }
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL);
        let id = m.in_flight.as_ref().unwrap().0;
        let mut cx = Cx::new(26_000);
        m.http_response(&mut cx, id, Err("timed out".into()));
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));

        let mut m = ready();
        let mut cx = Cx::new(3_000);
        m.timer(&mut cx, POLL);
        let a = answer(
            &mut m,
            3_010,
            "<camrply><result>err_reject</result></camrply>",
        );
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        let mut cx = Cx::new(3_020);
        m.command(&mut cx, 5, "capture", &Params::new());
        assert!(cx.take().contains(&Action::Complete {
            id: 5,
            result: Err(CommandError::NotConnected)
        }));
    }

    #[test]
    fn values_are_encoded_as_the_camera_expects() {
        assert_eq!(encode("2304/256"), "2304/256");
        assert_eq!(encode("-1/3"), "-1/3");
        assert_eq!(encode("My Mac"), "My%20Mac");
        assert_eq!(encode("+1"), "%2B1");
        assert_eq!(jpeg_in(&[1, 2, 3]), None);
    }
}
