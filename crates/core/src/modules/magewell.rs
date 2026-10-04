//! Magewell Pro Convert encoders and decoders over their HTTP APIs.
//!
//! From Magewell's Pro Convert Encoder API, Decoder API V1.3 and IP Decoder
//! API V1.2 (see the spec's sources):
//!
//! - Encoders and first-generation NDI decoders take `GET /mwapi?method=<name>`
//!   with the settings in the query, and answer JSON with an integer
//!   `status`, 0 for success. Logging in is `method=login&id=<user>&pass=<MD5
//!   of the password>`; the session ID comes back in a `Set-Cookie` header
//!   and goes on every later request as a cookie.
//! - The IP decoders take JSON under `/api/...`, also answering with a
//!   `status`. Logging in is `POST /api/user/login` with the SHA-256 of the
//!   password; the cookie is named after the serial number.
//! - Status 37 means the session has ended: the core logs in again and
//!   repeats the request once. Status 36 (and 16 on the IP decoders, an
//!   unknown user) refuses the credentials, which is final.
//! - Nothing is pushed: state comes from polling the summary.
//!
//! The session cookie is only in a response header, which the core's HTTP
//! requests do not return, so this module speaks HTTP/1.1 itself over TCP (or
//! TLS), one request per connection with `Connection: close`, one at a time.
//!
//! Opened for commands only, it polls nothing; `ping`, which needs no login,
//! is sent every 10 seconds as the liveness check.

use std::collections::VecDeque;
use std::net::SocketAddr;

use md5::{Digest as _, Md5};
use serde_json::{json, Map, Value};
use sha2::Sha256;

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Millis, Module, OpenContext, Outcome, TcpInput,
    TlsTarget,
};

const SOCKET: Key = "magewell-http";
const POLL: Key = "poll";
const REPLY: Key = "reply";

const POLL_EVERY: Millis = 2_000;
const PING_EVERY: Millis = 10_000;
const REPLY_TIMEOUT: Millis = 5_000;
/// More than any reply the API documents.
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

const NOT_LOGGED_IN: i64 = 37;
const AUTH_FAILED: i64 = 36;
const NO_SUCH_USER: i64 = 16;

/// Which API the model speaks.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Api {
    /// `/mwapi?method=...`: the encoders and first-generation NDI decoders.
    Mw,
    /// `/api/...`: the IP decoders.
    Rest,
}

#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Login,
    Poll,
    Ping,
    Command { id: CommandId, value: bool },
}

#[derive(Debug, Clone)]
struct Job {
    purpose: Purpose,
    method: &'static str,
    /// Path and query.
    target: String,
    body: Option<String>,
    /// Needs the session cookie.
    auth: bool,
    /// Already repeated after a fresh login.
    retried: bool,
}

/// A request on the wire and what has come back.
struct InFlight {
    job: Job,
    sent_at: Millis,
    received: Vec<u8>,
}

pub(crate) struct ProConvert {
    api: Api,
    device: SocketAddr,
    host: String,
    https: bool,
    username: String,
    password: String,
    monitor: bool,
    cookie: Option<String>,
    queue: VecDeque<Job>,
    in_flight: Option<InFlight>,
    refused: bool,
    connected: bool,
}

impl ProConvert {
    pub(crate) fn new(ctx: OpenContext) -> ProConvert {
        let text = |k: &str, d: &str| {
            ctx.settings
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or(d)
                .to_string()
        };
        let https = text("scheme", "http") == "https";
        let port = ctx.port.unwrap_or(if https { 443 } else { 80 });
        ProConvert {
            api: if ctx.model.starts_with("ip-") {
                Api::Rest
            } else {
                Api::Mw
            },
            device: SocketAddr::new(ctx.host, port),
            host: ctx.host.to_string(),
            https,
            username: text("username", "Admin"),
            password: text("password", "Admin"),
            monitor: ctx.monitor,
            cookie: None,
            queue: VecDeque::new(),
            in_flight: None,
            refused: false,
            connected: false,
        }
    }

    fn mw(method: &str, args: &[(&str, String)]) -> String {
        let mut t = format!("/mwapi?method={method}");
        for (k, v) in args {
            t.push('&');
            t.push_str(k);
            t.push('=');
            t.push_str(&encode(v));
        }
        t
    }

    fn login_job(&self) -> Job {
        match self.api {
            Api::Mw => {
                let hash = hex(&Md5::digest(self.password.as_bytes()));
                Job {
                    purpose: Purpose::Login,
                    method: "GET",
                    target: ProConvert::mw(
                        "login",
                        &[("id", self.username.clone()), ("pass", hash)],
                    ),
                    body: None,
                    auth: false,
                    retried: false,
                }
            }
            Api::Rest => {
                let hash = hex(&Sha256::digest(self.password.as_bytes()));
                Job {
                    purpose: Purpose::Login,
                    method: "POST",
                    target: "/api/user/login".into(),
                    body: Some(json!({"username": self.username, "password": hash}).to_string()),
                    auth: false,
                    retried: false,
                }
            }
        }
    }

    fn summary_job(&self, purpose: Purpose) -> Job {
        let target = match self.api {
            Api::Mw => ProConvert::mw("get-summary-info", &[]),
            Api::Rest => "/api/system/summary".into(),
        };
        Job {
            purpose,
            method: "GET",
            target,
            body: None,
            auth: true,
            retried: false,
        }
    }

    fn ping_job(&self, purpose: Purpose) -> Job {
        let target = match self.api {
            Api::Mw => ProConvert::mw("ping", &[]),
            Api::Rest => "/api/ping".into(),
        };
        Job {
            purpose,
            method: "GET",
            target,
            body: None,
            auth: false,
            retried: false,
        }
    }

    /// The request a command makes, or why it cannot.
    fn command_job(&self, id: CommandId, name: &str, p: &Params) -> Result<Job, CommandError> {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let i = |k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0);
        let f = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        let bs = |k: &str| b(k).to_string();
        let command = |value: bool| Purpose::Command { id, value };
        let unknown = || CommandError::UnknownCommand {
            command: name.into(),
        };
        if name == "ping" {
            return Ok(self.ping_job(command(false)));
        }
        if name == "get_summary" {
            return Ok(self.summary_job(command(true)));
        }
        match self.api {
            Api::Mw => {
                let (method, args, value): (&str, Vec<(&str, String)>, bool) = match name {
                    "reboot" => ("reboot", vec![], false),
                    "get_caps" => ("get-caps", vec![], true),
                    "get_signal_info" => ("get-signal-info", vec![], true),
                    "get_video_config" => ("get-video-config", vec![], true),
                    "get_ndi_config" => ("get-ndi-config", vec![], true),
                    "get_ndi_sources" => ("get-ndi-sources", vec![], true),
                    "get_tally_config" => ("get-tally", vec![], true),
                    "list_presets" => ("list-channels", vec![], true),
                    "get_current_source" => ("get-channel", vec![], true),
                    "get_video_modes" => ("get-supported-video-modes", vec![], true),
                    "get_audio_config" => ("get-audio-config", vec![], true),
                    "set_ndi_enabled" => ("set-ndi-config", vec![("enable", bs("enabled"))], false),
                    "set_ndi_source_name" => {
                        ("set-ndi-config", vec![("source-name", s("name"))], false)
                    }
                    "set_ndi_groups" => {
                        ("set-ndi-config", vec![("group-name", s("groups"))], false)
                    }
                    "set_ndi_transport" => {
                        let t = s("transport");
                        let on = |x: &str| (t == x).to_string();
                        (
                            "set-ndi-config",
                            vec![
                                ("enable-mcast", on("multicast")),
                                ("enable-rudp", on("rudp")),
                                ("enable-tcp", on("multi-tcp")),
                                ("enable-udp", on("udp")),
                            ],
                            false,
                        )
                    }
                    "set_ndi_multicast" => (
                        "set-ndi-config",
                        vec![
                            ("mcast-addr", s("address")),
                            ("mcast-mask", s("netmask")),
                            ("mcast-ttl", i("ttl").to_string()),
                        ],
                        false,
                    ),
                    "set_ndi_discovery" | "set_decoder_ndi_discovery" => (
                        "set-ndi-config",
                        vec![
                            ("enable-discovery", bs("enabled")),
                            ("discovery-server", s("servers")),
                        ],
                        false,
                    ),
                    "set_ndi_failover" => (
                        "set-ndi-config",
                        vec![
                            ("enable-fail-over", bs("enabled")),
                            ("fail-over-ndi-name", s("ndi_name")),
                            ("fail-over-ip-addr", s("ip_address")),
                        ],
                        false,
                    ),
                    "set_ndi_web_control" => (
                        "set-ndi-config",
                        vec![("enable-web-control", bs("enabled"))],
                        false,
                    ),
                    "set_ndi_ptz_control" => (
                        "set-ndi-config",
                        vec![("enable-ptz-control", bs("enabled"))],
                        false,
                    ),
                    "set_reference_level" => {
                        let level = if s("level") == "ebu" { "14" } else { "20" };
                        (
                            "set-ndi-config",
                            vec![("reference-level", level.into())],
                            false,
                        )
                    }
                    "set_decoder_ndi_group" => {
                        ("set-ndi-config", vec![("group-name", s("group"))], false)
                    }
                    "set_bitrate_ratio" => (
                        "set-video-config",
                        vec![("bit-rate-ratio", i("percent").to_string())],
                        false,
                    ),
                    "set_brightness" | "set_contrast" | "set_hue" | "set_saturation" => (
                        "set-video-config",
                        vec![(&name[4..], i("value").to_string())],
                        false,
                    ),
                    "set_deinterlace" => {
                        ("set-video-config", vec![("deinterlace", s("mode"))], false)
                    }
                    "set_output_flip" => {
                        ("set-video-config", vec![("out-flip", bs("enabled"))], false)
                    }
                    "set_output_mirror" => (
                        "set-video-config",
                        vec![("out-mirror", bs("enabled"))],
                        false,
                    ),
                    "set_output_resolution" => (
                        "set-video-config",
                        vec![
                            ("out-raw-resolution", "false".into()),
                            ("out-cx", i("width").to_string()),
                            ("out-cy", i("height").to_string()),
                        ],
                        false,
                    ),
                    "set_output_follows_input" => (
                        "set-video-config",
                        vec![("out-raw-resolution", bs("enabled"))],
                        false,
                    ),
                    "set_ext_tally" => ("set-tally", vec![("ext-tally", bs("enabled"))], false),
                    "select_ndi_source" => (
                        "set-channel",
                        vec![("ndi-name", "true".into()), ("name", s("name"))],
                        false,
                    ),
                    "select_preset" => (
                        "set-channel",
                        vec![("ndi-name", "false".into()), ("name", s("name"))],
                        false,
                    ),
                    "add_preset" => (
                        "add-channel",
                        vec![("name", s("name")), ("url", s("url"))],
                        false,
                    ),
                    "delete_preset" => ("del-channel", vec![("name", s("name"))], false),
                    "set_hdmi_output" => {
                        ("set-hdmi-output", vec![("enabled", bs("enabled"))], false)
                    }
                    "set_video_mode" => (
                        "set-video-mode",
                        vec![
                            ("width", i("width").to_string()),
                            ("height", i("height").to_string()),
                            ("interlaced", bs("interlaced")),
                            ("field-rate", i("field_rate").to_string()),
                            ("aspect-ratio", format!("{:.2}", f("aspect_ratio"))),
                        ],
                        false,
                    ),
                    "set_audio_gain" => (
                        "set-audio-config",
                        vec![("gain", format!("{:.2}", f("gain")))],
                        false,
                    ),
                    "set_overlay" => {
                        let key = match s("overlay").as_str() {
                            "title" => "show-title",
                            "tally" => "show-tally",
                            "vu-meter" => "show-vu-meter",
                            _ => "show-center-cross",
                        };
                        ("set-video-config", vec![(key, bs("visible"))], false)
                    }
                    "set_switch_mode" => {
                        ("set-video-config", vec![("switch-mode", s("mode"))], false)
                    }
                    "set_aspect_conversion" => (
                        "set-video-config",
                        vec![("ar-convert-mode", s("mode"))],
                        false,
                    ),
                    "set_decoder_flip" => (
                        "set-video-config",
                        vec![("h-flip", bs("horizontal")), ("v-flip", bs("vertical"))],
                        false,
                    ),
                    "set_buffer_duration" => (
                        "set-playback-config",
                        vec![("buffer-duration", i("ms").to_string())],
                        false,
                    ),
                    _ => return Err(unknown()),
                };
                Ok(Job {
                    purpose: command(value),
                    method: "GET",
                    target: ProConvert::mw(method, &args),
                    body: None,
                    auth: true,
                    retried: false,
                })
            }
            Api::Rest => {
                let (method, path, body, value): (&'static str, &str, Option<Value>, bool) =
                    match name {
                        "reboot" => ("POST", "/api/reboot", None, false),
                        "get_signal_info" => ("GET", "/api/signal/info", None, true),
                        "get_sources" => ("GET", "/api/source/list?type=all", None, true),
                        "select_source" => (
                            "POST",
                            "/api/source/select",
                            Some(json!({"id": i("id")})),
                            false,
                        ),
                        "restart_source" => ("POST", "/api/source/restart", Some(json!({})), false),
                        "list_profiles" => ("GET", "/api/profile/list", None, true),
                        "select_profile" => (
                            "POST",
                            "/api/profile/select",
                            Some(json!({"id": i("id")})),
                            false,
                        ),
                        "select_screen_source" => (
                            "POST",
                            "/api/profile/screen/select",
                            Some(
                                json!({"id": i("profile"), "screen-index": i("screen"), "source-id": i("source")}),
                            ),
                            false,
                        ),
                        "set_audio_gain" => (
                            "POST",
                            "/api/audio/config/set",
                            Some(json!({"gain": f("gain")})),
                            false,
                        ),
                        "ptz_move" => (
                            "POST",
                            "/api/ptz/move",
                            Some(json!({"pan": f("pan"), "tilt": f("tilt")})),
                            false,
                        ),
                        "ptz_zoom" => (
                            "POST",
                            "/api/ptz/zoom/set",
                            Some(json!({"speed": f("speed")})),
                            false,
                        ),
                        "ptz_focus" => (
                            "POST",
                            "/api/ptz/focus/set",
                            Some(json!({"speed": f("speed")})),
                            false,
                        ),
                        "ptz_auto_focus" => ("POST", "/api/ptz/focus/auto", Some(json!({})), false),
                        "ptz_recall_preset" => (
                            "POST",
                            "/api/ptz/preset/recall",
                            Some(json!({"number": i("number")})),
                            false,
                        ),
                        "ptz_store_preset" => (
                            "POST",
                            "/api/ptz/preset/store",
                            Some(json!({"number": i("number")})),
                            false,
                        ),
                        _ => return Err(unknown()),
                    };
                Ok(Job {
                    purpose: command(value),
                    method,
                    target: path.into(),
                    body: body.map(|b| b.to_string()),
                    auth: true,
                    retried: false,
                })
            }
        }
    }

    fn request_bytes(&self, job: &Job) -> Vec<u8> {
        let mut r = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
            job.method, job.target, self.host
        );
        if job.auth {
            if let Some(cookie) = &self.cookie {
                r.push_str(&format!("Cookie: {cookie}\r\n"));
            }
        }
        match &job.body {
            Some(body) => {
                r.push_str("Content-Type: application/json\r\n");
                r.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
            }
            None => r.push_str("\r\n"),
        }
        r.into_bytes()
    }

    /// Send the next queued request, logging in first when it needs a session.
    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.refused {
            return;
        }
        let Some(front) = self.queue.front() else {
            return;
        };
        if front.auth && self.cookie.is_none() {
            let login = self.login_job();
            self.queue.push_front(login);
        }
        let job = self.queue.pop_front().unwrap();
        self.in_flight = Some(InFlight {
            job,
            sent_at: cx.now(),
            received: Vec::new(),
        });
        if self.https {
            cx.tcp_open_tls(
                SOCKET,
                TlsTarget {
                    to: self.device,
                    server_name: self.host.clone(),
                    accept_invalid_certs: true,
                },
            );
        } else {
            cx.tcp_open(SOCKET, self.device);
        }
        cx.set_timer(REPLY, REPLY_TIMEOUT);
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        self.queue.push_back(job);
        self.pump(cx);
    }

    fn fail(&mut self, cx: &mut Cx, job: &Job, error: CommandError) {
        if let Purpose::Command { id, .. } = job.purpose {
            cx.complete(id, Err(error));
        }
    }

    /// The request failed on the network: report it and go on with the queue.
    fn transport_failed(&mut self, cx: &mut Cx, reason: String) {
        cx.cancel_timer(REPLY);
        let Some(flight) = self.in_flight.take() else {
            return;
        };
        cx.tcp_close(SOCKET);
        self.fail(
            cx,
            &flight.job,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        if flight.job.purpose == Purpose::Login {
            // Whatever waited on the login fails with it.
            for job in std::mem::take(&mut self.queue) {
                self.fail(
                    cx,
                    &job,
                    CommandError::Transport {
                        message: reason.clone(),
                    },
                );
            }
        }
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        self.pump(cx);
    }

    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        self.refused = true;
        for job in std::mem::take(&mut self.queue) {
            self.fail(
                cx,
                &job,
                CommandError::Auth {
                    message: reason.clone(),
                },
            );
        }
        cx.cancel_timer(POLL);
        cx.connection(Connection::Unauthorized { reason });
    }

    /// A whole response has arrived.
    fn answered(&mut self, cx: &mut Cx, flight: InFlight, response: Response) {
        cx.cancel_timer(REPLY);
        cx.alive();
        if !self.connected {
            self.connected = true;
            cx.connection(Connection::Connected);
        }
        let job = flight.job;
        let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
        let status = body.get("status").and_then(Value::as_i64);
        if (response.status == 401 || status == Some(NOT_LOGGED_IN))
            && job.purpose != Purpose::Login
            && !job.retried
        {
            // The session ended: log in again and repeat the request once.
            self.cookie = None;
            self.queue.push_front(Job {
                retried: true,
                ..job
            });
            return;
        }
        match &job.purpose {
            Purpose::Login => {
                if status == Some(0) {
                    let cookie = response.cookies();
                    self.cookie = Some(if cookie.is_empty() {
                        // No Set-Cookie: the IP decoder's reply also names
                        // the session.
                        match body.get("sid").and_then(Value::as_str) {
                            Some(sid) => format!("sid={sid}"),
                            None => String::new(),
                        }
                    } else {
                        cookie
                    });
                } else if matches!(status, Some(AUTH_FAILED) | Some(NO_SUCH_USER))
                    || response.status == 401
                    || response.status == 403
                {
                    self.refuse(
                        cx,
                        format!("login refused (status {})", status.unwrap_or(0)),
                    );
                } else {
                    let reason = format!(
                        "login failed (status {:?}, HTTP {})",
                        status, response.status
                    );
                    for job in std::mem::take(&mut self.queue) {
                        self.fail(
                            cx,
                            &job,
                            CommandError::DeviceError {
                                code: status.map(|s| s.to_string()),
                                message: reason.clone(),
                            },
                        );
                    }
                    cx.log(crate::module::Level::Warning, reason);
                }
            }
            Purpose::Poll => {
                if status == Some(0) {
                    self.apply_summary(cx, &body);
                }
            }
            Purpose::Ping => {
                cx.round_trip(cx.now().saturating_sub(flight.sent_at));
            }
            Purpose::Command { id, value } => {
                cx.round_trip(cx.now().saturating_sub(flight.sent_at));
                let id = *id;
                if response.status != 200 {
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: Some(response.status.to_string()),
                            message: format!("HTTP {}", response.status),
                        }),
                    );
                } else if status == Some(0) {
                    if job.target.contains("get-summary-info")
                        || job.target == "/api/system/summary"
                    {
                        self.apply_summary(cx, &body);
                    }
                    cx.complete(
                        id,
                        Ok(if *value {
                            Outcome::Value { value: body }
                        } else {
                            Outcome::Ack
                        }),
                    );
                } else {
                    let code = status.unwrap_or(-1);
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: Some(code.to_string()),
                            message: status_text(code).to_string(),
                        }),
                    );
                }
            }
        }
    }

    fn apply_summary(&self, cx: &mut Cx, body: &Value) {
        let mut patch = Map::new();
        match self.api {
            Api::Mw => {
                let d = &body["device"];
                let n = &body["ndi"];
                let mut device = Map::new();
                for (key, from) in [
                    ("model", "model"),
                    ("name", "name"),
                    ("serial", "serial-no"),
                    ("firmware", "fw-version"),
                    ("temperature", "core-temp"),
                    ("uptime", "up-time"),
                    ("input_state", "input-state"),
                    ("output_state", "output-state"),
                ] {
                    if let Some(v) = d.get(from).filter(|v| !v.is_null()) {
                        device.insert(key.into(), v.clone());
                    }
                }
                patch.insert("device".into(), Value::Object(device));
                let mut ndi = Map::new();
                for (key, from) in [
                    ("name", "name"),
                    ("url", "url"),
                    ("clients", "num-clients"),
                    ("video_kbps", "video-bit-rate"),
                ] {
                    if let Some(v) = n.get(from).filter(|v| !v.is_null()) {
                        ndi.insert(key.into(), v.clone());
                    }
                }
                if let (Some(w), Some(h)) = (n.get("video-width"), n.get("video-height")) {
                    let scan = n.get("video-scan").and_then(Value::as_str).unwrap_or("");
                    let rate = n
                        .get("video-field-rate")
                        .and_then(Value::as_f64)
                        .map(|r| format!("{r}"))
                        .unwrap_or_default();
                    ndi.insert(
                        "video".into(),
                        json!(format!("{w}x{h} {scan} {rate}").trim().to_string()),
                    );
                }
                patch.insert("ndi".into(), Value::Object(ndi));
                let mut tally = Map::new();
                for (key, from) in [("preview", "tally-preview"), ("program", "tally-program")] {
                    if let Some(v) = n.get(from).and_then(Value::as_bool) {
                        tally.insert(key.into(), json!(v));
                    }
                }
                patch.insert("tally".into(), Value::Object(tally));
            }
            Api::Rest => {
                let mut device = Map::new();
                for (key, from) in [
                    ("model", "product-name"),
                    ("name", "device-name"),
                    ("serial", "serial-number"),
                    ("firmware", "firmware-ver"),
                    ("temperature", "core-temp"),
                    ("uptime", "uptime"),
                ] {
                    if let Some(v) = body.get(from).filter(|v| !v.is_null()) {
                        device.insert(key.into(), v.clone());
                    }
                }
                if let Some(state) = body.get("hdmi-state").and_then(Value::as_i64) {
                    device.insert("output_state".into(), json!(state.to_string()));
                }
                patch.insert("device".into(), Value::Object(device));
                if let Some(stream) = body["profile"]["streams"]
                    .as_array()
                    .and_then(|s| s.first())
                {
                    let mut ndi = Map::new();
                    if let Some(name) = stream.get("name") {
                        ndi.insert("name".into(), name.clone());
                    }
                    if let Some(kbps) = stream["video"].get("kbps") {
                        ndi.insert("video_kbps".into(), kbps.clone());
                    }
                    patch.insert("ndi".into(), Value::Object(ndi));
                    let mut tally = Map::new();
                    for (key, from) in [("preview", "tally-preview"), ("program", "tally-program")]
                    {
                        if let Some(v) = stream["extra"].get(from).and_then(Value::as_bool) {
                            tally.insert(key.into(), json!(v));
                        }
                    }
                    patch.insert("tally".into(), Value::Object(tally));
                }
            }
        }
        cx.state(Value::Object(patch));
    }

    fn data(&mut self, cx: &mut Cx, data: &[u8]) {
        let Some(flight) = self.in_flight.as_mut() else {
            return;
        };
        flight.received.extend_from_slice(data);
        if flight.received.len() > MAX_RESPONSE {
            self.transport_failed(cx, "response too large".into());
            return;
        }
        if let Some(response) = Response::parse(&flight.received, false) {
            let flight = self.in_flight.take().unwrap();
            cx.tcp_close(SOCKET);
            self.answered(cx, flight, response);
            self.pump(cx);
        }
    }
}

/// An HTTP/1.1 response.
#[derive(Debug, PartialEq)]
struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// A complete response, or `None` while more is to come. `closed` takes
    /// whatever arrived as the whole body when there is no length.
    fn parse(data: &[u8], closed: bool) -> Option<Response> {
        let end = data.windows(4).position(|w| w == b"\r\n\r\n")?;
        let head = String::from_utf8_lossy(&data[..end]);
        let mut lines = head.split("\r\n");
        let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        let rest = &data[end + 4..];
        let header = |name: &str| {
            headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
        };
        let body = if header("transfer-encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
        {
            dechunk(rest)?
        } else if let Some(length) = header("content-length").and_then(|v| v.parse::<usize>().ok())
        {
            if rest.len() < length {
                return None;
            }
            rest[..length].to_vec()
        } else if closed {
            rest.to_vec()
        } else {
            return None;
        };
        Some(Response {
            status,
            headers,
            body,
        })
    }

    /// The `name=value` of each Set-Cookie, joined for a Cookie header.
    fn cookies(&self) -> String {
        self.headers
            .iter()
            .filter(|(k, _)| k == "set-cookie")
            .filter_map(|(_, v)| v.split(';').next())
            .map(|c| c.trim().to_string())
            .filter(|c| c.contains('='))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// A chunked body, once its last chunk has arrived.
fn dechunk(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n")?;
        let size_text = String::from_utf8_lossy(&data[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if data.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Percent-encoding for a query value, RFC 3986 unreserved characters kept.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The API's status codes (Encoder API section 1.3, Decoder API V1.3
/// section 1.4).
fn status_text(code: i64) -> &'static str {
    const NAMES: [&str; 56] = [
        "success",
        "pending",
        "timeout",
        "interrupted",
        "try again",
        "not implemented",
        "unknown error",
        "invalid argument",
        "no memory",
        "unsupported",
        "file busy",
        "device busy",
        "device lost",
        "I/O failed",
        "read failed",
        "write failed",
        "does not exist",
        "too many",
        "too large",
        "overflow",
        "underflow",
        "format error",
        "file exists",
        "file type error",
        "device type error",
        "is a directory",
        "read only",
        "range error",
        "broken pipe",
        "no space",
        "not a directory",
        "not permitted",
        "bad address",
        "seek error",
        "cross-device link",
        "not initialized",
        "authentication failed",
        "not logged in",
        "wrong state",
        "mismatch",
        "verify failed",
        "constraint violation",
        "canceled",
        "in progress",
        "connection refused",
        "connection reset",
        "address in use",
        "no response",
        "information changed",
        "invalid data",
        "need more data",
        "no buffer",
        "buffer too small",
        "buffer is empty",
        "buffer is full",
        "network unreachable",
    ];
    usize::try_from(code)
        .ok()
        .and_then(|c| NAMES.get(c))
        .copied()
        .unwrap_or("unknown status")
}

impl Module for ProConvert {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        if self.monitor {
            let job = self.summary_job(Purpose::Poll);
            self.enqueue(cx, job);
            cx.set_timer(POLL, POLL_EVERY);
        } else {
            let job = self.ping_job(Purpose::Ping);
            self.enqueue(cx, job);
            cx.set_timer(POLL, PING_EVERY);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "the device refused the credentials".into(),
                }),
            );
            return;
        }
        match self.command_job(id, name, params) {
            Ok(job) => self.enqueue(cx, job),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if let Some(flight) = &self.in_flight {
                    let bytes = self.request_bytes(&flight.job);
                    cx.tcp_send(SOCKET, bytes);
                }
            }
            TcpInput::Data(data) => self.data(cx, &data),
            TcpInput::Closed { reason } => {
                let Some(flight) = self.in_flight.as_ref() else {
                    return;
                };
                // A response without a length ends with the connection.
                match Response::parse(&flight.received, true) {
                    Some(response) => {
                        let flight = self.in_flight.take().unwrap();
                        self.answered(cx, flight, response);
                        self.pump(cx);
                    }
                    None => self.transport_failed(cx, reason),
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            POLL => {
                if self.refused {
                    return;
                }
                let (job, every) = if self.monitor {
                    (self.summary_job(Purpose::Poll), POLL_EVERY)
                } else {
                    (self.ping_job(Purpose::Ping), PING_EVERY)
                };
                // One at a time: a slow device is not sent a backlog.
                let pending = self
                    .queue
                    .iter()
                    .chain(self.in_flight.as_ref().map(|f| &f.job))
                    .any(|j| j.purpose == job.purpose);
                if !pending {
                    self.enqueue(cx, job);
                }
                cx.set_timer(POLL, every);
            }
            REPLY => self.transport_failed(cx, "no response within the timeout".into()),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() {
            cx.tcp_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(model: &str, monitor: bool) -> ProConvert {
        ProConvert::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 168, 66, 1)),
            port: None,
            model: model.into(),
            channels: None,
            settings: Params::new(),
            monitor,
        })
    }

    fn sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8_lossy(data).to_string()),
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

    /// Connect the pending request and return what was written.
    fn connect(m: &mut ProConvert, cx: &mut Cx) -> String {
        m.tcp(cx, SOCKET, TcpInput::Connected);
        sent(&cx.take_ref()).pop().unwrap_or_default()
    }

    trait TakeRef {
        fn take_ref(&mut self) -> Vec<Action>;
    }

    impl TakeRef for Cx {
        fn take_ref(&mut self) -> Vec<Action> {
            let now = self.now();
            std::mem::replace(self, Cx::new(now)).take()
        }
    }

    fn reply(body: &str, cookie: Option<&str>) -> Vec<u8> {
        let mut r = String::from("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n");
        if let Some(c) = cookie {
            r.push_str(&format!("Set-Cookie: {c}; path=/\r\n"));
        }
        r.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        r.into_bytes()
    }

    #[test]
    fn it_logs_in_with_the_md5_of_the_password_and_sends_the_cookie() {
        let mut m = module("hdmi-4k-plus", true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take_ref();
        let login = connect(&mut m, &mut cx);
        // MD5("Admin"), the document's example.
        assert!(login.starts_with(
            "GET /mwapi?method=login&id=Admin&pass=e3afed0047b08059d0fada10f400c1e5 HTTP/1.1\r\n"
        ));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(
                r#"{"status":0}"#,
                Some("sid=e0f6b33dd2b575eff40733b3778beaab"),
            )),
        );
        let poll = connect(&mut m, &mut cx);
        assert!(poll.starts_with("GET /mwapi?method=get-summary-info HTTP/1.1\r\n"));
        assert!(poll.contains("Cookie: sid=e0f6b33dd2b575eff40733b3778beaab\r\n"));
        let summary = r##"{"status":0,"device":{"name":"Pro Convert","model":"HDMI 4K Plus","serial-no":"B401180706020","fw-version":"1.1.72","input-state":"1920x1080p60","core-temp":61.5},"ndi":{"name":"#00 (B401180706020)","num-clients":2,"tally-preview":false,"tally-program":true,"video-width":1920,"video-height":1080,"video-scan":"progressive","video-field-rate":60.0,"video-bit-rate":125000}}"##;
        m.tcp(&mut cx, SOCKET, TcpInput::Data(reply(summary, None)));
        let s = state(&cx.take_ref());
        assert_eq!(s["device"]["model"], "HDMI 4K Plus");
        assert_eq!(s["device"]["input_state"], "1920x1080p60");
        assert_eq!(s["ndi"]["clients"], 2);
        assert_eq!(s["ndi"]["video"], "1920x1080 progressive 60");
        assert_eq!(s["tally"], json!({"preview": false, "program": true}));
    }

    #[test]
    fn a_command_logs_in_again_when_the_session_has_ended() {
        let mut m = module("ndi-to-hdmi", false);
        m.cookie = Some("sid=old".into());
        let mut cx = Cx::new(0);
        let p = json!({"name": "STUDIO (Camera 1)"})
            .as_object()
            .unwrap()
            .clone();
        m.command(&mut cx, 4, "select_ndi_source", &p);
        cx.take_ref();
        let first = connect(&mut m, &mut cx);
        assert!(first.starts_with(
            "GET /mwapi?method=set-channel&ndi-name=true&name=STUDIO%20%28Camera%201%29 HTTP/1.1"
        ));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":37}"#, None)),
        );
        let login = connect(&mut m, &mut cx);
        assert!(login.contains("method=login"));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":0}"#, Some("sid=new"))),
        );
        let again = connect(&mut m, &mut cx);
        assert!(again.contains("Cookie: sid=new"));
        cx = Cx::new(40);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":0}"#, None)),
        );
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Ack)
        }));
        assert!(a.contains(&Action::RoundTrip(40)));
    }

    #[test]
    fn a_refused_login_is_final() {
        let mut m = module("sdi-plus", true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take_ref();
        connect(&mut m, &mut cx);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":36}"#, None)),
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        let mut cx = Cx::new(10);
        m.command(&mut cx, 1, "reboot", &Params::new());
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                id: 1,
                result: Err(CommandError::Auth { .. })
            }]
        ));
    }

    #[test]
    fn commands_only_pings_without_logging_in() {
        let mut m = module("ndi-to-aio", false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take_ref();
        let ping = connect(&mut m, &mut cx);
        assert!(ping.starts_with("GET /mwapi?method=ping HTTP/1.1\r\n"));
        assert!(!ping.contains("Cookie"));
        let mut cx = Cx::new(15);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":0}"#, None)),
        );
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(15)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(state(&a) == json!({}));
    }

    #[test]
    fn the_ip_decoders_log_in_with_sha256_json() {
        let mut m = module("ip-to-hdmi", false);
        let mut cx = Cx::new(0);
        let p = json!({"number": 3}).as_object().unwrap().clone();
        m.command(&mut cx, 9, "ptz_recall_preset", &p);
        cx.take_ref();
        let login = connect(&mut m, &mut cx);
        assert!(login.starts_with("POST /api/user/login HTTP/1.1\r\n"));
        // SHA-256("Admin"), the document's example.
        assert!(login.ends_with(
            r#"{"username":"Admin","password":"c1c224b03cd9bc7b6a86d77f5dace40191766c485cd55dc48caf9ac873335d6f"}"#
        ));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(
                r#"{"status":0,"code":"Success","sid":"abc"}"#,
                Some("sid-A506220808450=abc"),
            )),
        );
        let recall = connect(&mut m, &mut cx);
        assert!(recall.starts_with("POST /api/ptz/preset/recall HTTP/1.1\r\n"));
        assert!(recall.contains("Cookie: sid-A506220808450=abc\r\n"));
        assert!(recall.ends_with(r#"{"number":3}"#));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(reply(r#"{"status":7,"code":"Invalid"}"#, None)),
        );
        assert!(cx.take().iter().any(|a| matches!(a,
            Action::Complete { id: 9, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "7")));
    }

    #[test]
    fn responses_are_read_by_length_chunks_or_close() {
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"st\r\n8\r\natus\":0}\r\n0\r\n\r\n";
        assert_eq!(
            Response::parse(chunked, false).unwrap().body,
            b"{\"status\":0}"
        );
        assert!(Response::parse(&chunked[..40], false).is_none());
        let bare = b"HTTP/1.0 200 OK\r\n\r\n{\"status\":0}";
        assert!(Response::parse(bare, false).is_none());
        assert_eq!(Response::parse(bare, true).unwrap().body, b"{\"status\":0}");
        assert_eq!(encode("a b/(c)"), "a%20b%2F%28c%29");
        assert_eq!(status_text(37), "not logged in");
    }
}
