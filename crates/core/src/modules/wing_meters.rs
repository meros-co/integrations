//! Behringer WING meters, added to the spec-driven console.
//!
//! Commands, queries and state are the spec's (`specs/behringer-wing.yaml`),
//! run by the spec engine over OSC. Meters are not in the OSC tree: the
//! console sends them only on its native interface (WING Remote Protocols
//! V3.0.6-27, "Channel 3: Metering", p.96-97, and "Communication Channels",
//! p.91-93).
//!
//! - A TCP connection to port 2222 selects channel 3 (`DF D3`), names the UDP
//!   port meters go to (`D3` and a big-endian word), a report id (`D4` and a
//!   big-endian long) and the meter collection (`DC`, type and index tokens,
//!   `DE`). Bytes are escaped as the document's transmit routine does.
//! - The console sends each block of meters to that port of this host as a
//!   datagram: the report id, then big-endian signed words, levels in 1/256
//!   dB, in the order the collection asked for them.
//! - Meter data stops after 5 seconds unless the report id is sent again, so
//!   it is renewed every 4; the TCP connection, idle otherwise, is kept by the
//!   same message (connections time out after 10 seconds, p.91).

use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    Bind, CommandId, Cx, HttpResponse, Key, Level, Millis, Module, OpenContext, RequestId,
    SseInput, TcpInput, WsInput,
};

const CONTROL: Key = "wing-meters-control";
const DATA: Key = "wing-meters-data";
const RENEW: Key = "wing-meters-renew";
const RECONNECT: Key = "wing-meters-reconnect";
/// The native interface's port (p.91).
const NATIVE_PORT: u16 = 2222;
/// Meter data stops after 5 s (p.96).
const RENEW_EVERY: Millis = 4_000;
/// The report id this client asks for its blocks under.
const REPORT_ID: u32 = 0x4d45_5452;
/// The port meters are sent to when the settings name none.
pub(crate) const DEFAULT_METER_PORT: u16 = 2224;

const ESCAPE: u8 = 0xdf;

/// One kind of strip in the collection: its type token (Table 4), how many
/// there are, its state key, and the meters each sends (Table 5).
struct Section {
    token: u8,
    count: u8,
    key: &'static str,
    meters: &'static [&'static str],
}

const STRIP: &[&str] = &[
    "input_left",
    "input_right",
    "output_left",
    "output_right",
    "gate_key",
    "gate_gain",
    "dyn_key",
    "dyn_gain",
];
const DCA: &[&str] = &[
    "pre_fader_left",
    "pre_fader_right",
    "post_fader_left",
    "post_fader_right",
];

const SECTIONS: &[Section] = &[
    Section {
        token: 0xa0,
        count: 40,
        key: "channels",
        meters: STRIP,
    },
    Section {
        token: 0xa1,
        count: 8,
        key: "auxes",
        meters: STRIP,
    },
    Section {
        token: 0xa2,
        count: 16,
        key: "buses",
        meters: STRIP,
    },
    Section {
        token: 0xa3,
        count: 4,
        key: "mains",
        meters: STRIP,
    },
    Section {
        token: 0xa4,
        count: 8,
        key: "matrices",
        meters: STRIP,
    },
    Section {
        token: 0xa5,
        count: 8,
        key: "dcas",
        meters: DCA,
    },
];

/// Bytes for a channel of the native interface, escaped as the document's
/// transmit routine does: an escape byte followed by a byte that would read
/// as a channel select (D0-DE) is sent as DF DE, and a trailing escape byte
/// is followed by DE.
fn escaped(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    let mut esc = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == ESCAPE {
            esc = true;
            out.push(b);
        } else if esc && (0xd0..=0xde).contains(&b) {
            // DF before a channel-select byte: DF DE, then the byte again.
            out.push(ESCAPE - 1);
            esc = false;
            continue;
        } else {
            esc = false;
            out.push(b);
        }
        i += 1;
    }
    if esc {
        out.push(ESCAPE - 1);
    }
    out
}

/// The request on channel 3: the UDP port, the report id and the
/// collection of every section's strips.
fn request(port: u16) -> Vec<u8> {
    let mut data = vec![0xd3];
    data.extend_from_slice(&port.to_be_bytes());
    data.push(0xd4);
    data.extend_from_slice(&REPORT_ID.to_be_bytes());
    data.push(0xdc);
    for s in SECTIONS {
        data.push(s.token);
        data.extend(0..s.count);
    }
    data.push(0xde);
    [vec![ESCAPE, 0xd3], escaped(&data)].concat()
}

/// The renewal: the report id again (p.96, "repeat this token with current
/// id to reset transmission timeout").
fn renewal() -> Vec<u8> {
    let mut data = vec![0xd4];
    data.extend_from_slice(&REPORT_ID.to_be_bytes());
    [vec![ESCAPE, 0xd3], escaped(&data)].concat()
}

/// A block of meters as a state patch: each word, a signed 1/256 dB value,
/// at its strip and meter. A block of another report id, or one shorter than
/// the collection, gives what it holds.
fn block(data: &[u8]) -> Option<Value> {
    if data.len() < 4 || data[..4] != REPORT_ID.to_be_bytes() {
        return None;
    }
    let mut words = data[4..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|w| f64::from(i16::from_be_bytes(*w)) / 256.0);
    let mut patch = Map::new();
    'all: for s in SECTIONS {
        let mut strips = Map::new();
        for n in 1..=s.count {
            let mut meters = Map::new();
            for name in s.meters {
                let Some(v) = words.next() else {
                    if !meters.is_empty() {
                        strips.insert(n.to_string(), json!({ "meters": meters }));
                    }
                    if !strips.is_empty() {
                        patch.insert(s.key.to_string(), Value::Object(strips));
                    }
                    break 'all;
                };
                meters.insert(name.to_string(), json!(v));
            }
            strips.insert(n.to_string(), json!({ "meters": meters }));
        }
        patch.insert(s.key.to_string(), Value::Object(strips));
    }
    (!patch.is_empty()).then_some(Value::Object(patch))
}

pub(crate) struct WingMeters {
    engine: SpecEngine,
    host: IpAddr,
    port: u16,
    enabled: bool,
    backoff: Millis,
}

impl WingMeters {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> WingMeters {
        let enabled = ctx.settings.get("meters").and_then(Value::as_bool) == Some(true);
        let port = ctx
            .settings
            .get("meter_port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(DEFAULT_METER_PORT);
        WingMeters {
            engine,
            host: ctx.host,
            port,
            enabled,
            backoff: 1_000,
        }
    }

    fn connect(&mut self, cx: &mut Cx) {
        cx.tcp_open(CONTROL, SocketAddr::new(self.host, NATIVE_PORT));
    }
}

impl Module for WingMeters {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        if self.enabled {
            cx.udp_open(DATA, Bind::Shared(self.port));
            self.connect(cx);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        if socket != DATA {
            return self.engine.datagram(cx, socket, from, data);
        }
        if let Some(patch) = block(data) {
            cx.state(patch);
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        match socket {
            DATA => cx.log(
                Level::Warning,
                format!("meters unavailable on UDP port {}: {message}", self.port),
            ),
            CONTROL => {
                cx.log(Level::Debug, format!("meter connection: {message}"));
                cx.set_timer(RECONNECT, self.backoff);
                self.backoff = (self.backoff * 2).min(30_000);
            }
            _ => self.engine.socket_error(cx, socket, message),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket != CONTROL {
            return self.engine.tcp(cx, socket, input);
        }
        match input {
            TcpInput::Connected => {
                self.backoff = 1_000;
                cx.tcp_send(CONTROL, request(self.port));
                cx.set_timer(RENEW, RENEW_EVERY);
            }
            // Nothing is asked on this connection but meters.
            TcpInput::Data(_) => {}
            TcpInput::Closed { .. } => {
                cx.cancel_timer(RENEW);
                cx.set_timer(RECONNECT, self.backoff);
                self.backoff = (self.backoff * 2).min(30_000);
            }
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.engine.http_response(cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RENEW => {
                cx.tcp_send(CONTROL, renewal());
                cx.set_timer(RENEW, RENEW_EVERY);
            }
            RECONNECT => self.connect(cx),
            _ => self.engine.timer(cx, key),
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.enabled {
            cx.tcp_close(CONTROL);
            cx.udp_close(DATA);
        }
        self.engine.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_are_escaped_as_the_transmit_routine_does() {
        // p.93: d702dfaf0e02 goes as it is; d702dfd10e02 becomes d702dfded10e02.
        assert_eq!(
            escaped(&[0xd7, 0x02, 0xdf, 0xaf, 0x0e, 0x02]),
            [0xd7, 0x02, 0xdf, 0xaf, 0x0e, 0x02]
        );
        assert_eq!(
            escaped(&[0xd7, 0x02, 0xdf, 0xd1, 0x0e, 0x02]),
            [0xd7, 0x02, 0xdf, 0xde, 0xd1, 0x0e, 0x02]
        );
        assert_eq!(escaped(&[0x01, 0xdf]), [0x01, 0xdf, 0xde]);
    }

    #[test]
    fn the_request_names_the_port_the_id_and_every_strip() {
        let r = request(0x3737);
        // p.97: dfd3d33737 declares port 14135, then d4 and the report id.
        assert_eq!(r[..5], [0xdf, 0xd3, 0xd3, 0x37, 0x37]);
        assert_eq!(r[5], 0xd4);
        assert_eq!(r[6..10], REPORT_ID.to_be_bytes());
        assert_eq!(r[10..13], [0xdc, 0xa0, 0x00]);
        assert_eq!(*r.last().unwrap(), 0xde);
        // Every strip's index token, each section's type token, the markers.
        let strips: usize = SECTIONS.iter().map(|s| s.count as usize).sum();
        assert_eq!(r.len(), 2 + 3 + 5 + 1 + SECTIONS.len() + strips + 1);
        assert_eq!(renewal(), [0xdf, 0xd3, 0xd4, 0x4d, 0x45, 0x54, 0x52]);
    }

    #[test]
    fn a_block_is_read_into_each_strips_meters() {
        let mut data = REPORT_ID.to_be_bytes().to_vec();
        // Channel 1: input left -10 dB, input right 0 dB, then the rest -128.
        for w in [-2560i16, 0] {
            data.extend_from_slice(&w.to_be_bytes());
        }
        let total: usize = SECTIONS
            .iter()
            .map(|s| s.count as usize * s.meters.len())
            .sum();
        for _ in 2..total {
            data.extend_from_slice(&i16::MIN.to_be_bytes());
        }
        let patch = block(&data).unwrap();
        assert_eq!(patch["channels"]["1"]["meters"]["input_left"], json!(-10.0));
        assert_eq!(patch["channels"]["1"]["meters"]["input_right"], json!(0.0));
        assert_eq!(patch["channels"]["40"]["meters"]["dyn_gain"], json!(-128.0));
        assert_eq!(
            patch["dcas"]["8"]["meters"]["post_fader_right"],
            json!(-128.0)
        );
        assert_eq!(patch["mains"].as_object().unwrap().len(), 4);
        // Another report id is not this client's.
        let mut other = data.clone();
        other[0] ^= 1;
        assert!(block(&other).is_none());
        // A short block gives what it holds.
        let short = block(&data[..4 + 2 * 3]).unwrap();
        assert_eq!(
            short,
            json!({"channels": {"1": {"meters": {
            "input_left": -10.0, "input_right": 0.0, "output_left": -128.0}}}})
        );
    }

    fn wing(settings: Value) -> WingMeters {
        use std::sync::Arc;
        let spec = crate::catalog::Catalog::source_tree()
            .device("behringer-wing")
            .unwrap()
            .clone();
        let ctx = OpenContext {
            host: IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "wing".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec), ctx.clone()).unwrap();
        WingMeters::new(engine, &ctx)
    }

    #[test]
    fn meters_are_asked_for_renewed_and_kept_only_when_turned_on() {
        use crate::module::Action;
        // Off: the console is only spoken to over OSC.
        let mut m = wing(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { .. })));

        let mut m = wing(json!({"meters": true, "meter_port": 14135}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::UdpOpen {
                socket: DATA,
                bind: Bind::Shared(14135)
            }
        )));
        assert!(a.iter().any(|a| matches!(a,
            Action::TcpOpen { socket: CONTROL, to } if to.port() == NATIVE_PORT)));
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, CONTROL, TcpInput::Connected);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a,
            Action::TcpSend { socket: CONTROL, data } if *data == request(14135))));
        let mut cx = Cx::new(4_010);
        m.timer(&mut cx, RENEW);
        assert!(cx.take().iter().any(|a| matches!(a,
            Action::TcpSend { socket: CONTROL, data } if *data == renewal())));
        let mut data = REPORT_ID.to_be_bytes().to_vec();
        data.extend_from_slice(&(-768i16).to_be_bytes());
        let mut cx = Cx::new(4_020);
        m.datagram(&mut cx, DATA, "127.0.0.1:2222".parse().unwrap(), &data);
        assert!(cx.take().iter().any(|a| matches!(a, Action::State(p)
            if p == &json!({"channels": {"1": {"meters": {"input_left": -3.0}}}}))));
    }
}
