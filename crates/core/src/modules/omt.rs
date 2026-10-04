//! Open Media Transport (OMT): a source's control and state, and the
//! discovery server's source list.
//!
//! From the OMT Protocol 1.0 document and Metadata Specification 1.0, with the
//! reference implementation (libomtnet, MIT) for what they leave out; see the
//! specs' sources:
//!
//! - Every message is a frame: a 16-byte little-endian header (version 1,
//!   frame type: metadata 1, video 2, audio 4; a timestamp; the per-frame
//!   metadata length; the data length, counting any extended header and
//!   per-frame metadata), then the data. Video and audio frames carry a 32 or
//!   24-byte extended header first. Control travels as metadata frames whose
//!   data is UTF-8 XML.
//! - A sender sends nothing a receiver did not subscribe to, except what it
//!   sends to each connection as it is accepted: its OMTInfo, its connection
//!   metadata (OMTWeb, OMTPTZ), the combined tally and any OMTRedirect. The
//!   core connects as a metadata-only receiver (`<OMTSubscribe
//!   Metadata="true" />`) and so takes no video or audio.
//! - Tally commands are four fixed strings, compared exactly by the sender;
//!   the sender combines every receiver's tally and sends the result to all
//!   subscribed receivers. A receiver's tally ends with its connection, so the
//!   last tally set here is sent again after every reconnection.
//! - Inband VISCA (`OMTPTZ Protocol="VISCA"`) carries a VISCA over IP command
//!   as hexadecimal with a sequence number, and the camera's reply comes back
//!   with the same number.
//! - The discovery server (TCP 6399) uses the same framing and announces each
//!   registered source as an `OMTAddress`, and its removal with `Removed`.
//!
//! Metadata is sent without a terminating NUL, as the reference
//! implementation sends it (it compares the whole frame data with its control
//! strings); received metadata may have one.
//!
//! Opened for commands only, a source session does not subscribe to metadata:
//! what the source sends on accepting the connection is still applied, later
//! changes are not received. OMT has no request a source must answer, so the
//! TCP connection is the only liveness signal in either mode. The discovery
//! session subscribes in both modes, since keeping the list is all it does.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Millis, Module, OpenContext, Outcome, TcpInput,
};

/// The first port a sender listens on (libomtnet `NETWORK_PORT_START`).
pub(crate) const SENDER_PORT: u16 = 6400;
/// The discovery server's default port (`DISCOVERY_SERVER_DEFAULT_PORT`).
pub(crate) const DISCOVERY_PORT: u16 = 6399;

const SOCKET: Key = "omt";
const REPLY: Key = "reply";
const RETRY: Key = "retry";

const HEADER: usize = 16;
const METADATA: u8 = 1;
const VIDEO: u8 = 2;
const AUDIO: u8 = 4;
/// Larger than any frame the reference implementation sends (10 MB video).
const MAX_FRAME: usize = 16 * 1024 * 1024;

const PTZ_TIMEOUT: Millis = 2_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const SUBSCRIBE_METADATA: &str = r#"<OMTSubscribe Metadata="true" />"#;

/// One metadata frame with no timestamp.
pub(crate) fn metadata_frame(xml: &str) -> Vec<u8> {
    let data = xml.as_bytes();
    let mut out = Vec::with_capacity(HEADER + data.len());
    out.push(1);
    out.push(METADATA);
    out.extend_from_slice(&0i64.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(data.len() as i32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// The tally string the sender recognises, exactly as the protocol writes
/// it, typo included.
pub(crate) fn tally_xml(preview: bool, program: bool) -> String {
    format!(
        r#"<OMTTally Preview="{preview}" Program=="{program}" />"#,
        preview = preview,
        program = program
    )
}

/// What a framed stream yields.
#[derive(Debug, PartialEq)]
enum Frame {
    Metadata(String),
    /// A video or audio frame, which a metadata-only receiver should not get.
    Media,
}

/// Splits the TCP stream into frames.
#[derive(Default)]
struct Reader {
    buf: Vec<u8>,
}

impl Reader {
    fn feed(&mut self, data: &[u8]) -> Result<Vec<Frame>, String> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < HEADER {
                return Ok(out);
            }
            let version = self.buf[0];
            let kind = self.buf[1];
            let length =
                i32::from_le_bytes([self.buf[12], self.buf[13], self.buf[14], self.buf[15]]);
            if version != 1 || !matches!(kind, METADATA | VIDEO | AUDIO) {
                return Err(format!("not an OMT frame (version {version}, type {kind})"));
            }
            if length < 0 || length as usize > MAX_FRAME {
                return Err(format!("frame length {length} out of range"));
            }
            let total = HEADER + length as usize;
            if self.buf.len() < total {
                return Ok(out);
            }
            let frame: Vec<u8> = self.buf.drain(..total).collect();
            if kind == METADATA {
                let text = String::from_utf8_lossy(&frame[HEADER..]);
                out.push(Frame::Metadata(text.trim_end_matches('\0').to_string()));
            } else {
                out.push(Frame::Media);
            }
        }
    }
}

/// Parses a metadata frame into (element name, attributes) pairs, looking
/// inside an `OMTGroup`. Text that is not well-formed XML yields nothing.
fn parse_elements(xml: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let Ok(doc) = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}').trim()) else {
        return Vec::new();
    };
    let root = doc.root_element();
    let attrs = |n: roxmltree::Node| {
        n.attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect::<BTreeMap<_, _>>()
    };
    if root.tag_name().name() == "OMTGroup" {
        root.children()
            .filter(|n| n.is_element())
            .map(|n| (n.tag_name().name().to_string(), attrs(n)))
            .collect()
    } else {
        vec![(root.tag_name().name().to_string(), attrs(root))]
    }
}

/// The tally a sender reports, from the fixed strings.
fn parse_tally(xml: &str) -> Option<(bool, bool)> {
    for preview in [false, true] {
        for program in [false, true] {
            if xml.trim() == tally_xml(preview, program) {
                return Some((preview, program));
            }
        }
    }
    None
}

/// A VISCA command waiting for its reply.
#[derive(Debug)]
struct Waiting {
    id: CommandId,
    sequence: u32,
    sent_at: Millis,
    deadline: Millis,
}

/// A session with one OMT source, as a metadata-only receiver.
pub(crate) struct Sender {
    device: SocketAddr,
    monitor: bool,
    reader: Reader,
    socket_open: bool,
    connected: bool,
    /// The tally this receiver last reported, sent again on reconnecting.
    tally: Option<(bool, bool)>,
    sequence: u32,
    waiting: VecDeque<Waiting>,
    retry_after: Millis,
}

impl Sender {
    pub(crate) fn new(ctx: OpenContext) -> Sender {
        Sender {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(SENDER_PORT)),
            monitor: ctx.monitor,
            reader: Reader::default(),
            socket_open: false,
            connected: false,
            tally: None,
            sequence: 0,
            waiting: VecDeque::new(),
            retry_after: RETRY_MIN,
        }
    }

    fn send(&self, cx: &mut Cx, xml: &str) {
        cx.tcp_send(SOCKET, metadata_frame(xml));
    }

    fn arm_reply(&self, cx: &mut Cx) {
        match self.waiting.iter().map(|w| w.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        for w in self.waiting.drain(..) {
            cx.complete(
                w.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        cx.cancel_timer(REPLY);
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.reader = Reader::default();
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn metadata(&mut self, cx: &mut Cx, xml: &str) {
        if let Some((preview, program)) = parse_tally(xml) {
            cx.state(json!({"tally": {"preview": preview, "program": program}}));
            return;
        }
        for (name, attrs) in parse_elements(xml) {
            let get = |k: &str| attrs.get(k).cloned().unwrap_or_default();
            match name.as_str() {
                "OMTInfo" => cx.state(json!({"info": {
                    "product_name": get("ProductName"),
                    "manufacturer": get("Manufacturer"),
                    "version": get("Version"),
                }})),
                "OMTWeb" => cx.state(json!({"web": {"url": get("URL")}})),
                "OMTRedirect" => cx.state(json!({"redirect": {"address": get("NewAddress")}})),
                "OMTPTZ" => {
                    let protocol = get("Protocol");
                    if let Some(reply) = attrs.get("Reply") {
                        let sequence = attrs
                            .get("Sequence")
                            .and_then(|s| s.trim().parse::<u32>().ok());
                        if let Some(i) = self
                            .waiting
                            .iter()
                            .position(|w| Some(w.sequence) == sequence)
                        {
                            let w = self.waiting.remove(i).unwrap();
                            cx.round_trip(cx.now().saturating_sub(w.sent_at));
                            cx.complete(
                                w.id,
                                Ok(Outcome::Value {
                                    value: Value::String(reply.to_uppercase()),
                                }),
                            );
                            self.arm_reply(cx);
                        }
                    } else if !attrs.contains_key("Command") {
                        // Connection metadata: which PTZ protocol the source offers.
                        cx.state(json!({"ptz": {"protocol": protocol, "url": get("URL")}}));
                    }
                }
                _ => {}
            }
        }
    }
}

impl Module for Sender {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, p: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        let text = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        match name {
            "set_tally" => {
                let tally = (flag("preview"), flag("program"));
                self.tally = Some(tally);
                self.send(cx, &tally_xml(tally.0, tally.1));
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "ptz_visca" => {
                let command = text("command").to_uppercase();
                self.sequence = self.sequence.wrapping_add(1);
                let sequence = self.sequence;
                self.send(
                    cx,
                    &format!(
                        r#"<OMTPTZ Protocol="VISCA" Sequence="{sequence}" Command="{command}" />"#
                    ),
                );
                self.waiting.push_back(Waiting {
                    id,
                    sequence,
                    sent_at: cx.now(),
                    deadline: cx.now() + PTZ_TIMEOUT,
                });
                self.arm_reply(cx);
            }
            "send_metadata" => {
                self.send(cx, &text("xml"));
                cx.complete(id, Ok(Outcome::Unverified));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                if self.monitor {
                    self.send(cx, SUBSCRIBE_METADATA);
                }
                // The source forgot this receiver's tally with the last
                // connection.
                if let Some((preview, program)) = self.tally {
                    self.send(cx, &tally_xml(preview, program));
                }
            }
            TcpInput::Data(data) => {
                cx.alive();
                match self.reader.feed(&data) {
                    Ok(frames) => {
                        for frame in frames {
                            if let Frame::Metadata(xml) = frame {
                                self.metadata(cx, &xml);
                            }
                        }
                    }
                    Err(reason) => self.lost(cx, reason),
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            REPLY => {
                let now = cx.now();
                while let Some(i) = self.waiting.iter().position(|w| w.deadline <= now) {
                    let w = self.waiting.remove(i).unwrap();
                    cx.complete(w.id, Err(CommandError::Timeout));
                }
                self.arm_reply(cx);
            }
            _ => {}
        }
    }
}

/// A session with an OMT discovery server, keeping its source list.
pub(crate) struct Discovery {
    server: SocketAddr,
    reader: Reader,
    socket_open: bool,
    /// Source names currently in state, so a lost connection can clear them.
    known: Vec<String>,
    retry_after: Millis,
}

impl Discovery {
    pub(crate) fn new(ctx: OpenContext) -> Discovery {
        Discovery {
            server: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DISCOVERY_PORT)),
            reader: Reader::default(),
            socket_open: false,
            known: Vec::new(),
            retry_after: RETRY_MIN,
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.reader = Reader::default();
        if !self.known.is_empty() {
            let mut gone = Map::new();
            for name in self.known.drain(..) {
                gone.insert(name, Value::Null);
            }
            cx.state(json!({ "sources": gone }));
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn address(&mut self, cx: &mut Cx, xml: &str) {
        let Ok(doc) = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}').trim()) else {
            return;
        };
        let root = doc.root_element();
        if root.tag_name().name() != "OMTAddress" {
            return;
        }
        let child = |name: &str| {
            root.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
        };
        let Some(name) = child("Name").filter(|n| !n.is_empty()) else {
            return;
        };
        let removed = child("Removed").is_some_and(|r| r.eq_ignore_ascii_case("true"));
        if removed {
            self.known.retain(|k| k != &name);
            cx.state(json!({ "sources": { name: Value::Null } }));
            return;
        }
        let Some(port) = child("Port").and_then(|p| p.parse::<u16>().ok()) else {
            return;
        };
        // The reference server writes IPAddress; the protocol document shows
        // Address.
        let addresses: Vec<String> = root
            .children()
            .filter(|n| n.has_tag_name("Addresses"))
            .flat_map(|a| a.children())
            .filter(|n| n.has_tag_name("IPAddress") || n.has_tag_name("Address"))
            .filter_map(|n| n.text().map(|t| t.trim().to_string()))
            .filter(|t| !t.is_empty())
            .collect();
        if !self.known.contains(&name) {
            self.known.push(name.clone());
        }
        cx.state(
            json!({ "sources": { name: { "port": port, "addresses": addresses.join(",") } } }),
        );
    }
}

impl Module for Discovery {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.server);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _p: &Params) {
        cx.complete(
            id,
            Err(CommandError::UnknownCommand {
                command: name.into(),
            }),
        );
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                // Keeping the list is all this session does, so it subscribes
                // whether or not it was opened to monitor.
                cx.tcp_send(SOCKET, metadata_frame(SUBSCRIBE_METADATA));
            }
            TcpInput::Data(data) => {
                cx.alive();
                match self.reader.feed(&data) {
                    Ok(frames) => {
                        for frame in frames {
                            if let Frame::Metadata(xml) = frame {
                                self.address(cx, &xml);
                            }
                        }
                    }
                    Err(reason) => self.lost(cx, reason),
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == RETRY {
            cx.connection(Connection::Connecting);
            cx.tcp_open(SOCKET, self.server);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn ctx(monitor: bool) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 60)),
            port: None,
            model: "sender".into(),
            channels: None,
            settings: Params::new(),
            monitor,
        }
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
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

    fn connected(monitor: bool) -> (Sender, Vec<Action>) {
        let mut m = Sender::new(ctx(monitor));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        (m, cx.take())
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn frames_follow_the_protocol_header() {
        let f = metadata_frame(SUBSCRIBE_METADATA);
        assert_eq!(f[0], 1);
        assert_eq!(f[1], METADATA);
        assert_eq!(&f[2..10], &[0; 8]);
        assert_eq!(&f[10..12], &[0, 0]);
        assert_eq!(
            i32::from_le_bytes([f[12], f[13], f[14], f[15]]) as usize,
            SUBSCRIBE_METADATA.len()
        );
        assert_eq!(&f[16..], SUBSCRIBE_METADATA.as_bytes());
        // Split anywhere, with a NUL-terminated frame and a media frame.
        let mut stream = metadata_frame("<OMTWeb URL=\"http://10.0.0.60/\" />");
        let mut nul = metadata_frame("<OMTInfo ProductName=\"P\" />\0");
        nul[12] = (nul.len() - HEADER) as u8;
        stream.extend(nul);
        let mut video = vec![1, VIDEO];
        video.extend_from_slice(&[0; 10]);
        video.extend_from_slice(&40i32.to_le_bytes());
        video.extend_from_slice(&[0; 40]);
        stream.extend(video);
        let mut r = Reader::default();
        let (a, b) = stream.split_at(7);
        assert!(r.feed(a).unwrap().is_empty());
        assert_eq!(
            r.feed(b).unwrap(),
            vec![
                Frame::Metadata("<OMTWeb URL=\"http://10.0.0.60/\" />".into()),
                Frame::Metadata("<OMTInfo ProductName=\"P\" />".into()),
                Frame::Media
            ]
        );
        assert!(Reader::default().feed(&[9; 16]).is_err());
    }

    #[test]
    fn connecting_subscribes_to_metadata_and_applies_what_the_source_sends() {
        let (mut m, a) = connected(true);
        assert_eq!(sent(&a), vec![metadata_frame(SUBSCRIBE_METADATA)]);
        let mut data = metadata_frame(
            r#"<OMTInfo ProductName="omtcapture" Manufacturer="OMT" Version="1.0" />"#,
        );
        data.extend(metadata_frame(r#"<OMTWeb URL="http://10.0.0.60/" />"#));
        data.extend(metadata_frame(
            r#"<OMTPTZ Protocol="VISCAoverIP" URL="visca://10.0.0.60:52381" />"#,
        ));
        data.extend(metadata_frame(&tally_xml(false, true)));
        data.extend(metadata_frame(
            r#"<OMTRedirect NewAddress="omt://host:6401" />"#,
        ));
        let mut cx = Cx::new(5);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data));
        assert_eq!(
            state(&cx.take()),
            json!({
                "info": {"product_name": "omtcapture", "manufacturer": "OMT", "version": "1.0"},
                "web": {"url": "http://10.0.0.60/"},
                "ptz": {"protocol": "VISCAoverIP", "url": "visca://10.0.0.60:52381"},
                "tally": {"preview": false, "program": true},
                "redirect": {"address": "omt://host:6401"}
            })
        );
    }

    #[test]
    fn tally_is_the_exact_string_and_is_sent_again_after_reconnecting() {
        let (mut m, _) = connected(true);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_tally",
            &params(json!({"preview": true, "program": false})),
        );
        let a = cx.take();
        assert_eq!(
            sent(&a),
            vec![metadata_frame(
                r#"<OMTTally Preview="true" Program=="false" />"#
            )]
        );
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Unverified)
        }));
        let mut cx = Cx::new(20);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(
            sent(&cx.take()),
            vec![
                metadata_frame(SUBSCRIBE_METADATA),
                metadata_frame(r#"<OMTTally Preview="true" Program=="false" />"#)
            ]
        );
    }

    #[test]
    fn a_visca_reply_answers_its_command_by_sequence() {
        let (mut m, _) = connected(true);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            7,
            "ptz_visca",
            &params(json!({"command": "8101040700ff"})),
        );
        assert_eq!(
            sent(&cx.take()),
            vec![metadata_frame(
                r#"<OMTPTZ Protocol="VISCA" Sequence="1" Command="8101040700FF" />"#
            )]
        );
        // Another sequence's reply is not this command's; a grouped one is.
        let mut cx = Cx::new(130);
        let mut data = metadata_frame(r#"<OMTPTZ Protocol="VISCA" Sequence="9" Reply="9041FF" />"#);
        data.extend(metadata_frame(
            r#"<OMTGroup><OMTPTZ Protocol="VISCA" Sequence="1" Reply="9051ff" /></OMTGroup>"#,
        ));
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data));
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(30)));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Value {
                value: json!("9051FF")
            })
        }));
        // Unanswered, it times out.
        let mut cx = Cx::new(200);
        m.command(
            &mut cx,
            8,
            "ptz_visca",
            &params(json!({"command": "8101040700FF"})),
        );
        let mut cx = Cx::new(200 + PTZ_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert!(cx.take().contains(&Action::Complete {
            id: 8,
            result: Err(CommandError::Timeout)
        }));
    }

    #[test]
    fn opened_for_commands_only_it_does_not_subscribe_but_commands_work() {
        let (mut m, a) = connected(false);
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // What the source sends on accepting the connection still applies.
        let mut cx = Cx::new(1);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(metadata_frame(&tally_xml(true, true))),
        );
        assert_eq!(
            state(&cx.take()),
            json!({"tally": {"preview": true, "program": true}})
        );
        let mut cx = Cx::new(2);
        m.command(
            &mut cx,
            3,
            "send_metadata",
            &params(json!({"xml": "<Custom />"})),
        );
        assert_eq!(sent(&cx.take()), vec![metadata_frame("<Custom />")]);
    }

    #[test]
    fn the_discovery_server_list_is_kept() {
        let mut d = Discovery::new(OpenContext {
            model: "server".into(),
            ..ctx(false)
        });
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        d.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert!(a.contains(&Action::TcpOpen {
            socket: SOCKET,
            to: "10.0.0.60:6399".parse().unwrap()
        }));
        assert_eq!(sent(&a), vec![metadata_frame(SUBSCRIBE_METADATA)]);
        let add = "<OMTAddress>\n  <Name>STUDIO1 (Camera 1)</Name>\n  <Port>6400</Port>\n  <Addresses>\n    <IPAddress>10.0.0.61</IPAddress>\n  </Addresses>\n</OMTAddress>";
        let mut cx = Cx::new(1);
        d.tcp(&mut cx, SOCKET, TcpInput::Data(metadata_frame(add)));
        assert_eq!(
            state(&cx.take()),
            json!({"sources": {"STUDIO1 (Camera 1)": {"port": 6400, "addresses": "10.0.0.61"}}})
        );
        let remove = "<OMTAddress><Name>STUDIO1 (Camera 1)</Name><Port>6400</Port><Removed>True</Removed><Addresses /></OMTAddress>";
        let mut cx = Cx::new(2);
        d.tcp(&mut cx, SOCKET, TcpInput::Data(metadata_frame(remove)));
        let a = cx.take();
        assert!(a.contains(&Action::State(
            json!({"sources": {"STUDIO1 (Camera 1)": null}})
        )));
        // Lost: everything known is cleared.
        let mut cx = Cx::new(3);
        d.tcp(&mut cx, SOCKET, TcpInput::Data(metadata_frame(add)));
        d.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        assert!(cx.take().contains(&Action::State(
            json!({"sources": {"STUDIO1 (Camera 1)": null}})
        )));
    }
}
