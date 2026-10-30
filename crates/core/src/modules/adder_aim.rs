//! Adder AdderLink Infinity Manager (A.I.M.) servers over the AIM API
//! (version 14, AIM 5.10), as appendix K of the A.I.M. user guide documents
//! it: `GET /api/?v=<version>&method=<name>&token=<token>&...`, answered with
//! an XML `<api_response>` holding `success` (1 or 0), the method's values
//! and, on failure, `<errors><error><code/><msg/></error></errors>`.
//!
//! - The module logs in once (`login` with the user name and password, or
//!   none for the anonymous API user) and keeps the token for every request,
//!   as Adder asks: each token is a session the AIM stores until it expires.
//!   A refused user name or password (error 2) is terminal: the device is
//!   unauthorized until its settings change. A request refused for a login
//!   reason (an error naming the login, the token or the session) logs in
//!   again once and is sent again; the API documents no code for it.
//! - Monitored, the receivers and transmitters (`get_devices`, every page,
//!   all receivers whatever the user's permissions), channels, presets and
//!   C-USB LAN extenders are read every `poll_interval_ms` (5 s by default,
//!   within the 5 to 10 s Adder recommends), the AIM servers and disk usage
//!   every minute, and everything again after a command that changes them.
//! - Requests go one at a time. A server that does not answer is reported
//!   disconnected and asked again at the next poll.
//!
//! Opened for commands only (`monitor` false), the module logs in (commands
//! need the token) and reads nothing; a `get_servers` every 30 s of silence
//! stays as the liveness check. On closing it logs out, ending its session.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::IpAddr;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, Outcome, PollStamps, RequestId,
};

const POLL_TIMER: Key = "poll";
const SLOW_TIMER: Key = "slow";
const RETRY_TIMER: Key = "retry";
const SLOW_EVERY: Millis = 60_000;
const LIVENESS: Millis = 30_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const PAGE: u64 = 1000;

/// Percent-encode a query value (RFC 3986 unreserved characters kept).
pub(crate) fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// One element of an XML answer, as JSON: text-only children by their name
/// (lowercased), elements with children as objects, and an element's
/// `item` attribute when it has one.
fn element_json(node: roxmltree::Node) -> Value {
    let mut map = Map::new();
    if let Some(item) = node.attribute("item") {
        map.insert("item".into(), json!(item));
    }
    for child in node.children().filter(|c| c.is_element()) {
        let name = child.tag_name().name().to_ascii_lowercase();
        let has_elements = child.children().any(|c| c.is_element());
        let value = if has_elements {
            element_json(child)
        } else {
            json!(child.text().unwrap_or("").trim())
        };
        map.insert(name, value);
    }
    Value::Object(map)
}

/// An answer: whether it succeeded, its errors, and the document.
#[derive(Debug)]
pub(crate) struct Answer {
    pub success: bool,
    pub errors: Vec<(String, String)>,
    pub doc: String,
}

pub(crate) fn parse_answer(body: &[u8]) -> Option<Answer> {
    let text = String::from_utf8_lossy(body).into_owned();
    let doc = roxmltree::Document::parse(&text).ok()?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("api_response") {
        return None;
    }
    let child_text = |name: &str| {
        root.children()
            .find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(name))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
    };
    let success = child_text("success").as_deref() == Some("1");
    let errors = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("error"))
        .map(|e| {
            let get = |name: &str| {
                e.children()
                    .find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(name))
                    .and_then(|c| c.text())
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };
            (get("code"), get("msg"))
        })
        .collect();
    Some(Answer {
        success,
        errors,
        doc: text,
    })
}

/// The values of an answer's elements of one name (devices, channels...),
/// each as JSON, and the root's own text values.
fn items(doc: &str, name: &str) -> (Vec<Value>, Map<String, Value>) {
    let Ok(doc) = roxmltree::Document::parse(doc) else {
        return (Vec::new(), Map::new());
    };
    let root = doc.root_element();
    let list = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name))
        .map(element_json)
        .collect();
    let mut top = Map::new();
    for c in root.children().filter(|c| c.is_element()) {
        if !c.children().any(|x| x.is_element()) {
            top.insert(
                c.tag_name().name().to_ascii_lowercase(),
                json!(c.text().unwrap_or("").trim()),
            );
        }
    }
    (list, top)
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn int(v: &Value, k: &str) -> Option<i64> {
    v.get(k)
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
}

fn flag(v: &Value, k: &str) -> Option<bool> {
    match v.get(k).and_then(Value::as_str)? {
        "1" | "true" => Some(true),
        "0" | "false" | "" => Some(false),
        _ => None,
    }
}

/// d_status (the database schema's On-line Status).
fn device_status(code: i64) -> &'static str {
    match code {
        0 => "offline",
        1 => "online",
        2 => "rebooting",
        3 => "factory_resetting",
        4 => "firmware_upgrading",
        5 => "online_unconfigured",
        6 => "running_backup_firmware",
        7 => "id_no_longer_online",
        10 => "rdp_device",
        _ => "unknown",
    }
}

/// What a request is for.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Login,
    /// A page of a list read: which, and the page.
    List(List, u64),
    Disk,
    Liveness,
    Command(CommandId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum List {
    Receivers,
    Transmitters,
    Channels,
    Presets,
    CUsb,
    Servers,
}

#[derive(Debug, Clone)]
struct Request {
    query: Vec<(String, String)>,
    purpose: Purpose,
    /// Sent again once after a login, when refused for a login reason.
    retried: bool,
}

pub(crate) struct AdderAim {
    base: String,
    accept_invalid_certs: bool,
    username: String,
    password: String,
    version: u64,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_every: Millis,
    timeout: Millis,
    token: Option<String>,
    refused: bool,
    reachable: bool,
    retry_after: Millis,
    queue: VecDeque<Request>,
    in_flight: Option<(RequestId, Request, Millis)>,
    next_id: RequestId,
    heard_at: Millis,
    /// Pages of a list gathered until its last page: its rows by id.
    gathering: BTreeMap<List, Map<String, Value>>,
    /// The ids each list last held, to remove those gone.
    known: HashMap<List, Vec<String>>,
}

impl AdderAim {
    pub(crate) fn new(ctx: OpenContext) -> Result<AdderAim, String> {
        if ctx.host.is_unspecified() && ctx.host_name.is_none() {
            return Err("give the AIM server's address as host".into());
        }
        let s = &ctx.settings;
        let scheme = match s.get("scheme").and_then(Value::as_str) {
            Some("https") => "https",
            _ => "http",
        };
        let port = ctx.port.unwrap_or(if scheme == "https" { 443 } else { 80 });
        let host = match (&ctx.host_name, ctx.host) {
            (Some(n), _) => n.clone(),
            (None, IpAddr::V6(a)) => format!("[{a}]"),
            (None, a) => a.to_string(),
        };
        let st = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let num = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        Ok(AdderAim {
            base: format!("{scheme}://{host}:{port}/api/"),
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            username: st("username"),
            password: st("password"),
            version: num("api_version", 14),
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_every: num("poll_interval_ms", 5000).max(500),
            timeout: num("timeout_ms", 10_000).max(1),
            token: None,
            refused: false,
            reachable: false,
            retry_after: RETRY_MIN,
            queue: VecDeque::new(),
            in_flight: None,
            next_id: 0,
            heard_at: 0,
            gathering: BTreeMap::new(),
            known: HashMap::new(),
        })
    }

    fn url(&self, query: &[(String, String)]) -> String {
        let mut url = format!("{}?v={}", self.base, self.version);
        for (k, v) in query {
            url.push_str(&format!("&{k}={}", encode(v)));
        }
        if let Some(token) = &self.token {
            url.push_str(&format!("&token={}", encode(token)));
        }
        url
    }

    fn login_request(&self) -> Request {
        let mut query = vec![("method".to_string(), "login".to_string())];
        // No user name: the anonymous API user's token.
        if !self.username.is_empty() {
            query.push(("username".into(), self.username.clone()));
            query.push(("password".into(), self.password.clone()));
        }
        Request {
            query,
            purpose: Purpose::Login,
            retried: false,
        }
    }

    fn list_request(list: List, page: u64) -> Request {
        let mut query: Vec<(String, String)> = match list {
            List::Receivers => vec![
                ("method".into(), "get_devices".into()),
                ("device_type".into(), "rx".into()),
                ("show_all".into(), "1".into()),
            ],
            List::Transmitters => vec![
                ("method".into(), "get_devices".into()),
                ("device_type".into(), "tx".into()),
            ],
            List::Channels => vec![("method".into(), "get_channels".into())],
            List::Presets => vec![("method".into(), "get_presets".into())],
            List::CUsb => vec![("method".into(), "get_all_c_usb".into())],
            List::Servers => vec![("method".into(), "get_servers".into())],
        };
        if !matches!(list, List::CUsb) {
            query.push(("page".into(), page.to_string()));
            query.push(("results_per_page".into(), PAGE.to_string()));
        }
        Request {
            query,
            purpose: Purpose::List(list, page),
            retried: false,
        }
    }

    fn queue_read(&mut self, list: List) {
        let waiting = self
            .queue
            .iter()
            .any(|r| matches!(r.purpose, Purpose::List(l, _) if l == list))
            || self
                .in_flight
                .as_ref()
                .is_some_and(|(_, r, _)| matches!(r.purpose, Purpose::List(l, _) if l == list));
        if !waiting {
            self.queue.push_back(AdderAim::list_request(list, 1));
        }
    }

    fn sweep(&mut self) {
        for list in [
            List::Receivers,
            List::Transmitters,
            List::Channels,
            List::Presets,
            List::CUsb,
        ] {
            self.queue_read(list);
        }
    }

    fn sweep_slow(&mut self) {
        self.queue_read(List::Servers);
        if !self.queue.iter().any(|r| r.purpose == Purpose::Disk) {
            self.queue.push_back(Request {
                query: vec![("method".into(), "disk_usage".into())],
                purpose: Purpose::Disk,
                retried: false,
            });
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.refused {
            return;
        }
        // Nothing but the login goes without a token.
        if self.token.is_none()
            && !matches!(self.queue.front(), Some(r) if r.purpose == Purpose::Login)
        {
            if self.queue.iter().any(|r| r.purpose == Purpose::Login) {
                return;
            }
            self.queue.push_front(self.login_request());
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        self.next_id += 1;
        let url = self.url(&request.query);
        cx.http(
            self.next_id,
            HttpRequest {
                method: "GET",
                url,
                headers: Vec::new(),
                body: None,
                body_stream: None,
                timeout: Some(self.timeout),
                accept_invalid_certs: self.accept_invalid_certs,
                digest: None,
            },
        );
        self.in_flight = Some((self.next_id, request, cx.now()));
    }

    fn enqueue_command(&mut self, cx: &mut Cx, id: CommandId, query: Vec<(String, String)>) {
        let request = Request {
            query,
            purpose: Purpose::Command(id),
            retried: false,
        };
        let at = self
            .queue
            .iter()
            .position(|r| !matches!(r.purpose, Purpose::Command(_) | Purpose::Login));
        match at {
            Some(at) => self.queue.insert(at, request),
            None => self.queue.push_back(request),
        }
        self.pump(cx);
    }

    fn fail_all(&mut self, cx: &mut Cx, error: CommandError) {
        for r in self.queue.drain(..) {
            if let Purpose::Command(id) = r.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
    }

    /// An error that says the token is no good: log in again.
    fn login_error(errors: &[(String, String)]) -> bool {
        errors.iter().any(|(_, msg)| {
            let m = msg.to_ascii_lowercase();
            ["login", "logged", "token", "session", "authenticat"]
                .iter()
                .any(|w| m.contains(w))
        })
    }

    fn apply_list(&mut self, cx: &mut Cx, list: List, page: u64, answer: &Answer) {
        let (element, total_key, count_key) = match list {
            List::Receivers | List::Transmitters => ("device", "total_devices", "count_devices"),
            List::Channels => ("channel", "total_channels", "count_channels"),
            List::Presets => ("connection_preset", "total_presets", "count_presets"),
            List::CUsb => ("c_usb", "total_c_usbs", "count_c_usbs"),
            List::Servers => ("server", "total_servers", "count_servers"),
        };
        let (rows, top) = items(&answer.doc, element);
        let gathered = self.gathering.entry(list).or_default();
        for row in &rows {
            let (key, value) = match list {
                List::Receivers | List::Transmitters => {
                    (text(row, "d_id"), device_json(row, list == List::Receivers))
                }
                List::Channels => (text(row, "c_id"), channel_json(row)),
                List::Presets => (text(row, "cp_id"), preset_json(row)),
                List::CUsb => (text(row, "mac"), c_usb_json(row)),
                List::Servers => (text(row, "item"), server_json(row)),
            };
            if !key.is_empty() {
                gathered.insert(key, value);
            }
        }
        let total = top
            .get(total_key)
            .and_then(Value::as_str)
            .and_then(|t| t.parse::<u64>().ok());
        let count = top
            .get(count_key)
            .and_then(Value::as_str)
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or(rows.len() as u64);
        let more = matches!(total, Some(t) if page * PAGE < t) && count >= PAGE;
        if more {
            self.queue
                .push_front(AdderAim::list_request(list, page + 1));
            return;
        }
        let rows = self.gathering.remove(&list).unwrap_or_default();
        let key = match list {
            List::Receivers => "receivers",
            List::Transmitters => "transmitters",
            List::Channels => "channels",
            List::Presets => "presets",
            List::CUsb => "c_usb",
            List::Servers => "servers",
        };
        let mut patch = rows.clone();
        let ids: Vec<String> = rows.keys().cloned().collect();
        for gone in self
            .known
            .get(&list)
            .into_iter()
            .flatten()
            .filter(|k| !rows.contains_key(*k))
        {
            patch.insert(gone.clone(), Value::Null);
        }
        self.known.insert(list, ids);
        cx.state(json!({ key: patch }));
    }

    fn answered(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some((current, _, _)) = &self.in_flight else {
            return;
        };
        if *current != id {
            return;
        }
        let (_, request, sent_at) = self.in_flight.take().expect("in flight");
        let response = match result {
            Ok(r) => r,
            Err(message) => {
                if let Purpose::Command(cid) = request.purpose {
                    cx.complete(
                        cid,
                        Err(CommandError::Transport {
                            message: message.clone(),
                        }),
                    );
                }
                if self.reachable {
                    self.reachable = false;
                    cx.connection(Connection::Disconnected { reason: message });
                }
                // Commands waiting would fail the same way; polls come again.
                self.fail_all(
                    cx,
                    CommandError::Transport {
                        message: "the AIM server did not answer".into(),
                    },
                );
                self.queue.retain(|r| r.purpose == Purpose::Login);
                if self.token.is_none() && request.purpose == Purpose::Login {
                    cx.set_timer(RETRY_TIMER, self.retry_after);
                    self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
                }
                return;
            }
        };
        cx.round_trip(cx.now().saturating_sub(sent_at));
        cx.alive();
        self.heard_at = cx.now();
        if !self.reachable && request.purpose != Purpose::Login {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let Some(answer) = parse_answer(&response.body) else {
            let message = format!("HTTP {}: not an AIM API answer", response.status);
            if let Purpose::Command(cid) = request.purpose {
                cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: None,
                        message,
                    }),
                );
            } else {
                cx.log(Level::Warning, message);
            }
            return self.pump(cx);
        };
        match &request.purpose {
            Purpose::Login => {
                let token = items(&answer.doc, "x")
                    .1
                    .get("token")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                match (answer.success, token) {
                    (true, Some(token)) if !token.is_empty() => {
                        self.token = Some(token);
                        self.retry_after = RETRY_MIN;
                        if !self.reachable {
                            self.reachable = true;
                            cx.connection(Connection::Connected);
                        }
                        let top = items(&answer.doc, "x").1;
                        cx.state(json!({"api": {"version": top.get("version").cloned().unwrap_or(json!(""))}}));
                    }
                    _ => {
                        let reason = answer
                            .errors
                            .first()
                            .map(|(c, m)| format!("the AIM refused the login: {m} ({c})"))
                            .unwrap_or_else(|| "the AIM refused the login".into());
                        self.refused = true;
                        self.fail_all(
                            cx,
                            CommandError::Auth {
                                message: reason.clone(),
                            },
                        );
                        cx.connection(Connection::Unauthorized { reason });
                        return;
                    }
                }
            }
            _ if !answer.success && AdderAim::login_error(&answer.errors) && !request.retried => {
                // The token ended: log in again, then send it once more.
                self.token = None;
                let mut again = request.clone();
                again.retried = true;
                self.queue.push_front(again);
                self.queue.push_front(self.login_request());
            }
            Purpose::List(list, page) => {
                if answer.success {
                    self.apply_list(cx, *list, *page, &answer);
                    if self.monitor {
                        let group = if *list == List::Servers {
                            "slow"
                        } else {
                            "status"
                        };
                        self.stamps.answered(cx, group);
                    }
                }
            }
            Purpose::Disk => {
                if answer.success {
                    let top = items(&answer.doc, "x").1;
                    let mut disk = Map::new();
                    for (k, name) in [
                        ("mainusedpercent", "main_used_percent"),
                        ("backupusedpercent", "backup_used_percent"),
                        ("dbsize", "db_size"),
                        ("dbeventlogsize", "db_event_log_size"),
                        ("dbconnectionlogsize", "db_connection_log_size"),
                        ("dpconnectionlogsize", "db_connection_log_size"),
                        ("backupssize", "backups_size"),
                        ("archivessize", "archives_size"),
                        ("firmwaresize", "firmware_size"),
                        ("debugsize", "debug_size"),
                        ("aimupgradessize", "aim_upgrades_size"),
                        ("aimupgradesize", "aim_upgrades_size"),
                    ] {
                        if let Some(v) = top
                            .get(k)
                            .and_then(Value::as_str)
                            .and_then(|t| t.parse::<f64>().ok())
                        {
                            disk.insert(name.into(), json!(v));
                        }
                    }
                    cx.state(json!({ "disk": disk }));
                }
            }
            Purpose::Command(cid) => {
                let cid = *cid;
                if answer.success {
                    let (_, top) = items(&answer.doc, "x");
                    let value = match top.get("id") {
                        Some(id) => json!({"id": id}),
                        None => json!(true),
                    };
                    cx.complete(cid, Ok(Outcome::Value { value }));
                    // What the command changed is read again.
                    if self.monitor {
                        self.sweep();
                    }
                } else {
                    let (code, message) =
                        answer.errors.first().cloned().unwrap_or_else(|| {
                            (String::new(), "the AIM refused the request".into())
                        });
                    cx.complete(
                        cid,
                        Err(CommandError::DeviceError {
                            code: (!code.is_empty()).then_some(code),
                            message,
                        }),
                    );
                }
            }
            Purpose::Liveness => {}
        }
        self.pump(cx);
    }
}

fn device_json(d: &Value, receiver: bool) -> Value {
    let status = int(d, "d_status").unwrap_or(-1);
    let mut v = json!({
        "name": text(d, "d_name"),
        "description": text(d, "d_description"),
        "location": text(d, "d_location"),
        "status": device_status(status),
        "online": status == 1,
        "ip_address": text(d, "d_ip_address"),
        "ip_address2": text(d, "d_ip_address2"),
        "mac_address": text(d, "d_mac_address"),
        "mac_address2": text(d, "d_mac_address2"),
        "serial_number": text(d, "d_serial_number"),
        "firmware": text(d, "d_firmware"),
        "backup_firmware": text(d, "d_backup_firmware"),
        "version": int(d, "d_version"),
        "variant": text(d, "d_variant"),
        "configured": flag(d, "d_configured"),
        "valid_firmware": flag(d, "d_valid_firmware"),
        "date_added": text(d, "d_date_added"),
    });
    if receiver {
        // A connection is current while it has no end time.
        let channel = text(d, "con_c_id");
        let connected = !channel.is_empty() && text(d, "con_end_time").is_empty();
        v["connected"] = json!(connected);
        v["channel"] = json!(if connected { channel } else { String::new() });
        v["channel_name"] = json!(if connected {
            text(d, "c_name")
        } else {
            String::new()
        });
        v["mode"] = json!(
            match (connected, int(d, "con_control"), flag(d, "con_exclusive")) {
                (false, _, _) => "none",
                (true, _, Some(true)) => "private",
                (true, Some(1), _) => "video_only",
                (true, Some(2), _) => "exclusive",
                (true, Some(3), _) => "shared",
                _ => "unknown",
            }
        );
        v["user"] = json!(text(d, "u_username"));
        v["connection_start"] = json!(text(d, "con_start_time"));
        v["connection_end"] = json!(text(d, "con_end_time"));
    } else {
        v["channel_count"] = json!(int(d, "count_transmitter_channels"));
    }
    v
}

fn channel_json(c: &Value) -> Value {
    json!({
        "name": text(c, "c_name"),
        "description": text(c, "c_description"),
        "location": text(c, "c_location"),
        "type": text(c, "c_channel_type"),
        "transmitter": text(c, "c_tx_id"),
        "online": flag(c, "channel_online"),
        "favourite": flag(c, "c_favourite"),
        "video1": text(c, "c_video1"),
        "video1_head": text(c, "c_video1_head"),
        "video2": text(c, "c_video2"),
        "video2_head": text(c, "c_video2_head"),
        "audio": text(c, "c_audio"),
        "usb": text(c, "c_usb"),
        "serial": text(c, "c_serial"),
        "sensitive": flag(c, "c_sensitive"),
        "view": text(c, "view_button"),
        "shared": text(c, "shared_button"),
        "control": text(c, "control_button"),
        "exclusive": text(c, "exclusive_button"),
    })
}

fn preset_json(p: &Value) -> Value {
    json!({
        "name": text(p, "cp_name"),
        "description": text(p, "cp_description"),
        "pairs": int(p, "cp_pairs"),
        "problem_pairs": int(p, "problem_cp_pairs").unwrap_or(0),
        "active": text(p, "cp_active"),
        "connected_receivers": int(p, "connected_rx_count").unwrap_or(0),
        "view": text(p, "view_button"),
        "shared": text(p, "shared_button"),
        "control": text(p, "control_button"),
        "exclusive": text(p, "exclusive_button"),
    })
}

fn c_usb_json(c: &Value) -> Value {
    json!({
        "type": text(c, "type"),
        "name": text(c, "name"),
        "online": flag(c, "online"),
        "ip_address": text(c, "ip"),
        "connected_to": text(c, "connectedto"),
    })
}

fn server_json(s: &Value) -> Value {
    json!({
        "name": text(s, "name"),
        "role": text(s, "role"),
        "status": text(s, "status"),
        "ip_address": text(s, "ip"),
        "mac_address": text(s, "mac"),
        "eth1": int(s, "eth1"),
        "ip_address2": text(s, "ip2"),
        "mac_address2": text(s, "mac2"),
        "description": text(s, "description"),
        "location": text(s, "location"),
    })
}

fn mode_letter(params: &Params) -> String {
    match params.get("mode").and_then(Value::as_str) {
        Some("video_only") => "v",
        Some("exclusive") => "e",
        Some("private") => "p",
        _ => "s",
    }
    .into()
}

fn allowed_letters(params: &Params) -> Option<String> {
    let mut out = String::new();
    for (k, l) in [
        ("allow_video_only", 'v'),
        ("allow_shared", 's'),
        ("allow_exclusive", 'e'),
        ("allow_private", 'p'),
    ] {
        if params.get(k).and_then(Value::as_bool).unwrap_or(false) {
            out.push(l);
        }
    }
    (!out.is_empty()).then_some(out)
}

impl Module for AdderAim {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        if self.monitor {
            self.sweep();
            self.sweep_slow();
            cx.set_timer(POLL_TIMER, self.poll_every);
            cx.set_timer(SLOW_TIMER, SLOW_EVERY);
        } else {
            cx.set_timer(POLL_TIMER, LIVENESS);
        }
        self.pump(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "the AIM refused the login".into(),
                }),
            );
            return;
        }
        let s = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let n = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
                .unwrap_or_default()
        };
        let b =
            |k: &str| u8::from(params.get(k).and_then(Value::as_bool).unwrap_or(false)).to_string();
        let m = |method: &str| ("method".to_string(), method.to_string());
        let mut q: Vec<(String, String)> = match name {
            "refresh" => {
                self.sweep();
                self.sweep_slow();
                self.pump(cx);
                cx.complete(id, Ok(Outcome::Ack));
                return;
            }
            "connect_channel" => vec![
                m("connect_channel"),
                ("c_id".into(), n("channel")),
                ("rx_id".into(), n("receiver")),
                ("mode".into(), mode_letter(params)),
            ],
            "disconnect_receiver" => vec![
                m("disconnect_channel"),
                ("rx_id".into(), s("receivers")),
                ("force".into(), b("force")),
            ],
            "disconnect_all_receivers" => {
                vec![m("disconnect_channel"), ("force".into(), b("force"))]
            }
            "connect_preset" => vec![
                m("connect_preset"),
                ("id".into(), n("preset")),
                ("mode".into(), mode_letter(params)),
                ("force".into(), b("force")),
            ],
            "disconnect_preset" => vec![
                m("disconnect_preset"),
                ("id".into(), n("preset")),
                ("force".into(), b("force")),
            ],
            "disconnect_everything" => vec![m("disconnect_preset"), ("force".into(), b("force"))],
            "create_preset" => {
                let mut q = vec![
                    m("create_preset"),
                    ("name".into(), s("name")),
                    ("pairs".into(), s("pairs")),
                ];
                if let Some(a) = allowed_letters(params) {
                    q.push(("allowed".into(), a));
                }
                q
            }
            "delete_preset" => vec![m("delete_preset"), ("id".into(), n("preset"))],
            "create_channel" => {
                let mut q = vec![m("create_channel"), ("name".into(), s("name"))];
                for (k, key) in [
                    ("description", "desc"),
                    ("location", "loc"),
                    ("group", "groupname"),
                ] {
                    if !s(k).is_empty() {
                        q.push((key.into(), s(k)));
                    }
                }
                if let Some(a) = allowed_letters(params) {
                    q.push(("allowed".into(), a));
                }
                for (k, key) in [
                    ("video1", "video1"),
                    ("video1_head", "video1head"),
                    ("video2", "video2"),
                    ("video2_head", "video2head"),
                    ("audio", "audio"),
                    ("usb", "usb"),
                    ("serial", "serial"),
                ] {
                    if params.get(k).and_then(Value::as_u64).is_some_and(|v| v > 0) {
                        q.push((key.into(), n(k)));
                    }
                }
                if !q.iter().any(|(k, _)| {
                    ["video1", "video2", "audio", "usb", "serial"].contains(&k.as_str())
                }) {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "a channel needs at least one source device".into(),
                        }),
                    );
                    return;
                }
                q
            }
            "delete_channel" => vec![m("delete_channel"), ("id".into(), n("channel"))],
            "update_device" => {
                let mut q = vec![m("update_device"), ("id".into(), n("device"))];
                // Empty keeps the field; "_" clears it, as the API takes it.
                for (k, key) in [("description", "desc"), ("location", "loc")] {
                    match params.get(k).and_then(Value::as_str) {
                        Some("") | None => {}
                        Some(v) => q.push((key.into(), v.into())),
                    }
                }
                for (k, key) in [("clear_description", "desc"), ("clear_location", "loc")] {
                    if params.get(k).and_then(Value::as_bool) == Some(true) {
                        q.retain(|(x, _)| x != key);
                        q.push((key.into(), "_".into()));
                    }
                }
                q
            }
            "reboot_devices" => vec![m("reboot_devices"), ("ids".into(), s("devices"))],
            "identify_device" => vec![m("identify_device"), ("id".into(), n("device"))],
            "replace_device" => vec![
                m("replace_device"),
                ("d_id".into(), n("device")),
                ("r_d_id".into(), n("replacement")),
            ],
            "logout_receiver" => vec![m("logout_device"), ("rx_id".into(), n("receiver"))],
            "promote" => vec![m("promote")],
            "rename_c_usb" => vec![
                m("update_c_usb"),
                ("mac".into(), s("mac")),
                ("name".into(), s("name")),
            ],
            "delete_c_usb" => vec![m("delete_c_usb"), ("mac".into(), s("mac"))],
            "connect_c_usb" => vec![
                m("connect_c_usb"),
                ("rx".into(), s("receiver")),
                ("tx".into(), s("transmitter")),
            ],
            "disconnect_c_usb" => vec![m("disconnect_c_usb"), ("mac".into(), s("mac"))],
            other => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        };
        q.retain(|(_, v)| !v.is_empty());
        self.enqueue_command(cx, id, q);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.answered(cx, id, result);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused {
            return;
        }
        match key {
            RETRY_TIMER => self.pump(cx),
            POLL_TIMER => {
                if self.monitor {
                    self.sweep();
                    cx.set_timer(POLL_TIMER, self.poll_every);
                } else {
                    if self.in_flight.is_none()
                        && self.queue.is_empty()
                        && cx.now().saturating_sub(self.heard_at) >= LIVENESS
                    {
                        let mut r = AdderAim::list_request(List::Servers, 1);
                        r.purpose = Purpose::Liveness;
                        self.queue.push_back(r);
                    }
                    cx.set_timer(POLL_TIMER, LIVENESS);
                }
                self.pump(cx);
            }
            SLOW_TIMER if self.monitor => {
                self.sweep_slow();
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                self.pump(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        // End the session the token holds, as Adder asks of clients.
        if self.token.is_some() {
            self.next_id += 1;
            let url = self.url(&[("method".into(), "logout".into())]);
            cx.http(
                self.next_id,
                HttpRequest {
                    method: "GET",
                    url,
                    headers: Vec::new(),
                    body: None,
                    body_stream: None,
                    timeout: Some(self.timeout),
                    accept_invalid_certs: self.accept_invalid_certs,
                    digest: None,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::Ipv4Addr;

    fn module(monitor: bool, settings: Value) -> AdderAim {
        let spec = Catalog::source_tree().devices["adder-aim"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        AdderAim::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 50)),
            host_name: None,
            port: None,
            model: "aim".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, String)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((*id, request.url.clone())),
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn reply(m: &mut AdderAim, now: Millis, body: &str) -> Vec<Action> {
        let (id, _, _) = m.in_flight.clone().expect("a request in flight");
        let mut cx = Cx::new(now);
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

    const LOGIN_OK: &str = "<api_response><version>14</version><timestamp>2012-12-14 12:12:12</timestamp><success>1</success><token>5cf494a71c29e9465a57a81e0a2d602c</token></api_response>";
    const BASE: &str = "http://192.0.2.50:80/api/?v=14";

    #[test]
    fn it_logs_in_once_and_sends_the_token() {
        let mut m = module(false, json!({"username": "admin", "password": "p w"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert_eq!(
            requests(&a)[0].1,
            format!("{BASE}&method=login&username=admin&password=p%20w")
        );
        let a = reply(&mut m, 4, LOGIN_OK);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(4)));
        // Commands only: nothing is read.
        assert!(requests(&a).is_empty());
        let mut cx = Cx::new(5);
        m.command(
            &mut cx,
            1,
            "connect_channel",
            json!({"receiver": 2, "channel": 1, "mode": "exclusive"})
                .as_object()
                .unwrap(),
        );
        // The manual's example: connect_channel&c_id=1&rx_id=2&mode=e.
        assert_eq!(
            requests(&cx.take())[0].1,
            format!(
                "{BASE}&method=connect_channel&c_id=1&rx_id=2&mode=e&token=5cf494a71c29e9465a57a81e0a2d602c"
            )
        );
        let a = reply(
            &mut m,
            6,
            "<api_response><version>2</version><timestamp>x</timestamp><success>0</success><errors><error><code>231</code><msg>ERROR - private connection not available</msg></error></errors></api_response>",
        );
        assert_eq!(
            completed(&a, 1),
            Some(Err(CommandError::DeviceError {
                code: Some("231".into()),
                message: "ERROR - private connection not available".into()
            }))
        );
    }

    #[test]
    fn a_refused_login_is_terminal() {
        let mut m = module(true, json!({"username": "admin", "password": "bad"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = reply(
            &mut m,
            1,
            "<api_response><version>1</version><timestamp>x</timestamp><success>0</success><errors><error><code>2</code><msg>Invalid username or password</msg></error></errors></api_response>",
        );
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(requests(&a).is_empty());
    }

    #[test]
    fn monitored_it_reads_the_lists_into_state() {
        let mut m = module(true, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        // The anonymous user: no credentials.
        assert_eq!(requests(&cx.take())[0].1, format!("{BASE}&method=login"));
        let a = reply(&mut m, 1, LOGIN_OK);
        assert!(requests(&a)[0]
            .1
            .contains("method=get_devices&device_type=rx&show_all=1&page=1&results_per_page=1000"));
        // The manual's get_devices example, a receiver connected to channel 9.
        let a = reply(
            &mut m,
            2,
            "<api_response><version>2</version><timestamp>2012-09-12 14:56:11</timestamp><success>1</success><page>1</page><results_per_page>1000</results_per_page><total_devices>1</total_devices><count_devices>1</count_devices><devices><device item=\"1\"><d_id>170</d_id><d_serial_number>1409A0000159</d_serial_number><d_mac_address>00:0F:58:01:6E:3D</d_mac_address><d_mac_address2>00:0F:58:5B:6E:3D</d_mac_address2><d_name>RX 123</d_name><d_online>1</d_online><d_online2>0</d_online2><d_type>rx</d_type><d_version>2</d_version><d_variant></d_variant><d_ip_address0/><d_ip_address>10.10.10.66</d_ip_address><d_ip_address2>10.10.10.67</d_ip_address2><d_description></d_description><d_location>Server Rack 3</d_location><d_configured>1</d_configured><d_valid_firmware>1</d_valid_firmware><d_valid_backup_firmware>1</d_valid_backup_firmware><d_firmware>2.3.16682</d_firmware><d_backup_firmware>2.3.16682</d_backup_firmware><d_date_added>2012-07-14 01:37:07</d_date_added><d_status>1</d_status><d_domain_no>0</d_domain_no><con_c_id>9</con_c_id><con_exclusive>0</con_exclusive><con_control>1</con_control><con_start_time>2012-09-07 13:33:19</con_start_time><con_end_time/><u_username>admin</u_username><u_id>1</u_id><c_name>Channel 1</c_name><count_receiver_groups>1</count_receiver_groups><count_receiver_presets>2</count_receiver_presets><count_users>1</count_users></device></devices></api_response>",
        );
        let rx = &state(&a)["receivers"]["170"];
        assert_eq!(rx["name"], "RX 123");
        assert_eq!(rx["online"], true);
        assert_eq!(rx["status"], "online");
        assert_eq!(rx["connected"], true);
        assert_eq!(rx["channel"], "9");
        assert_eq!(rx["channel_name"], "Channel 1");
        assert_eq!(rx["mode"], "video_only");
        assert_eq!(rx["location"], "Server Rack 3");
        // Transmitters next, then a preset list.
        assert!(requests(&a)[0].1.contains("device_type=tx"));
        reply(
            &mut m,
            3,
            "<api_response><success>1</success><total_devices>0</total_devices><count_devices>0</count_devices></api_response>",
        );
        reply(
            &mut m,
            4,
            "<api_response><success>1</success><count_channels>0</count_channels></api_response>",
        );
        let a = reply(
            &mut m,
            5,
            "<api_response><version>1</version><timestamp>x</timestamp><success>1</success><page>1</page><results_per_page>10</results_per_page><total_presets>1</total_presets><count_presets>1</count_presets><connection_preset item=\"1\"><cp_id>3</cp_id><cp_name>Preset 1</cp_name><cp_description>Description for Preset 1</cp_description><cp_pairs>1</cp_pairs><problem_cp_pairs/><cp_active>full</cp_active><connected_rx_count>1</connected_rx_count><view_button>disabled</view_button><shared_button>disabled</shared_button><control_button>disabled</control_button><exclusive_button>disabled</exclusive_button></connection_preset></api_response>",
        );
        let p = &state(&a)["presets"]["3"];
        assert_eq!(p["name"], "Preset 1");
        assert_eq!(p["active"], "full");
        assert_eq!(p["connected_receivers"], 1);
        assert_eq!(p["problem_pairs"], 0);
    }

    #[test]
    fn an_ended_token_logs_in_again_and_repeats_the_request() {
        let mut m = module(false, json!({"username": "admin"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        reply(&mut m, 1, LOGIN_OK);
        let mut cx = Cx::new(2);
        m.command(
            &mut cx,
            7,
            "identify_device",
            json!({"device": 101}).as_object().unwrap(),
        );
        let a = reply(
            &mut m,
            3,
            "<api_response><success>0</success><errors><error><code>1</code><msg>You must be logged in</msg></error></errors></api_response>",
        );
        assert!(requests(&a)[0].1.contains("method=login"));
        let a = reply(&mut m, 4, LOGIN_OK);
        assert!(requests(&a)[0].1.contains("method=identify_device&id=101"));
        let a = reply(
            &mut m,
            5,
            "<api_response><version>7</version><timestamp>x</timestamp><success>1</success></api_response>",
        );
        assert_eq!(
            completed(&a, 7),
            Some(Ok(Outcome::Value { value: json!(true) }))
        );
    }

    #[test]
    fn values_are_percent_encoded() {
        assert_eq!(encode("John's Desk"), "John%27s%20Desk");
        assert_eq!(encode("1-1,1-2"), "1-1%2C1-2");
    }
}
