//! Art-Net 4 (Artistic Licence, Art-Net 4 specification revision 1.4dp): the
//! core as an Art-Net controller on UDP 6454.
//!
//! - One socket on the Art-Net port (`Bind::Network`, so other Art-Net
//!   software on this host can listen too), hearing every node. ArtPoll goes
//!   to the poll addresses (2.255.255.255 and 10.255.255.255 by default, the
//!   specification's default poll) and to the host when one is given, every
//!   3 s as the specification requires of controllers, asking for an
//!   ArtPollReply whenever a node's conditions change. Each ArtPollReply is
//!   kept in state under `nodes.<ip>/<bind index>`; a node that does not
//!   answer a poll before the next is marked offline.
//! - DMX output is held per universe (Port-Address) and sent as ArtDmx,
//!   unicast to every node subscribed to the universe (listing it in SwIn or
//!   SwOut) and to the host when one is given, as Art-Net 4 requires: never
//!   broadcast. A change is sent at once, at most one frame per 23 ms (44 Hz),
//!   and every held universe is sent again each `refresh_ms`. With `sync` on,
//!   each burst is followed by an ArtSync.
//! - ArtDmx from other controllers or input nodes is kept in state for the
//!   universes `receive` names, at most every `receive_interval_ms`.
//! - Node configuration (ArtAddress, ArtInput) completes when the node's
//!   ArtPollReply comes back; ArtIpProg, ArtDataRequest and ArtRdm complete
//!   with their replies. ArtTodData is kept under `rdm.<universe>`;
//!   ArtDiagData, ArtTimeCode, ArtCommand, ArtTrigger, ArtSync, ArtNzs and
//!   ArtRdm are reported as message events too.
//! - The core answers every ArtPoll with an ArtPollReply of its own, as the
//!   specification requires of every Art-Net device, and ignores its own
//!   packets heard back.
//! - Opening for commands only (monitor false) still polls, since sending
//!   DMX needs the subscribers polling finds and the protocol requires a
//!   controller to poll; it keeps no received DMX in state.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

pub(crate) const PORT: u16 = 0x1936;
const SOCKET: Key = "artnet";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const REFRESH: Key = "refresh";
const FLUSH: Key = "flush";
const DEADLINE: Key = "deadline";

const ID: &[u8; 8] = b"Art-Net\0";
const PROTOCOL: u16 = 14;
/// Controllers poll every 2.5 to 3 s; a node silent for that long after a
/// poll is taken as gone (ArtPoll).
const POLL_EVERY: Millis = 3000;
/// The least time between two DMX frames of a universe: 44 Hz, DMX512's
/// fastest.
const FRAME_GAP: Millis = 23;
/// How long a node is given to answer a command (ArtPoll's 3 s).
const ANSWER: Millis = 3000;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;
/// Own broadcasts heard back within this long are dropped.
const ECHO: Millis = 2000;

pub(crate) mod op {
    pub const POLL: u16 = 0x2000;
    pub const POLL_REPLY: u16 = 0x2100;
    pub const DIAG_DATA: u16 = 0x2300;
    pub const COMMAND: u16 = 0x2400;
    pub const DATA_REQUEST: u16 = 0x2700;
    pub const DATA_REPLY: u16 = 0x2800;
    pub const DMX: u16 = 0x5000;
    pub const NZS: u16 = 0x5100;
    pub const SYNC: u16 = 0x5200;
    pub const ADDRESS: u16 = 0x6000;
    pub const INPUT: u16 = 0x7000;
    pub const TOD_REQUEST: u16 = 0x8000;
    pub const TOD_DATA: u16 = 0x8100;
    pub const TOD_CONTROL: u16 = 0x8200;
    pub const RDM: u16 = 0x8300;
    pub const IP_PROG: u16 = 0xf800;
    pub const IP_PROG_REPLY: u16 = 0xf900;
    pub const TIME_CODE: u16 = 0x9700;
    pub const TRIGGER: u16 = 0x9900;
}

/// The packet header: ID, OpCode (low byte first), and for every packet but
/// ArtPollReply the protocol version (high byte first).
fn header(opcode: u16) -> Vec<u8> {
    let mut out = ID.to_vec();
    out.extend_from_slice(&opcode.to_le_bytes());
    out.extend_from_slice(&PROTOCOL.to_be_bytes());
    out
}

/// A fixed-length, null-terminated text field.
fn text_field(out: &mut Vec<u8>, text: &str, len: usize) {
    let mut bytes: Vec<u8> = text.bytes().take(len - 1).collect();
    bytes.resize(len, 0);
    out.extend_from_slice(&bytes);
}

fn read_text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end])
        .trim_end()
        .to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
}

/// The ArtPoll: Flags, DiagPriority, no target range, then the ESTA and OEM
/// codes.
pub(crate) fn art_poll(flags: u8, priority: u8, esta: u16, oem: u16) -> Vec<u8> {
    let mut out = header(op::POLL);
    out.push(flags);
    out.push(priority);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&esta.to_be_bytes());
    out.extend_from_slice(&oem.to_be_bytes());
    out
}

pub(crate) fn art_dmx(sequence: u8, universe: u16, data: &[u8]) -> Vec<u8> {
    let mut out = header(op::DMX);
    out.push(sequence);
    out.push(0); // Physical
    out.push((universe & 0xff) as u8);
    out.push((universe >> 8) as u8 & 0x7f);
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
    out
}

pub(crate) fn art_nzs(sequence: u8, start_code: u8, universe: u16, data: &[u8]) -> Vec<u8> {
    let mut out = header(op::NZS);
    out.push(sequence);
    out.push(start_code);
    out.push((universe & 0xff) as u8);
    out.push((universe >> 8) as u8 & 0x7f);
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
    out
}

pub(crate) fn art_sync() -> Vec<u8> {
    let mut out = header(op::SYNC);
    out.extend_from_slice(&[0, 0]);
    out
}

/// What an ArtAddress changes; each field is sent as the specification
/// wants it (bit 7 set to program, 0x7f for no change, 0 to reset).
pub(crate) struct Address {
    pub net: u8,
    pub bind_index: u8,
    pub short_name: String,
    pub long_name: String,
    pub sw_in: [u8; 4],
    pub sw_out: [u8; 4],
    pub sub: u8,
    pub acn_priority: u8,
    pub command: u8,
}

/// "No change" for a switch field: bit 7 clear, and not zero (zero resets).
pub(crate) const UNCHANGED: u8 = 0x7f;

impl Address {
    pub(crate) fn unchanged(bind_index: u8) -> Address {
        Address {
            net: UNCHANGED,
            bind_index,
            short_name: String::new(),
            long_name: String::new(),
            sw_in: [UNCHANGED; 4],
            sw_out: [UNCHANGED; 4],
            sub: UNCHANGED,
            acn_priority: 255,
            command: 0,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = header(op::ADDRESS);
        out.push(self.net);
        out.push(self.bind_index);
        text_field(&mut out, &self.short_name, 18);
        text_field(&mut out, &self.long_name, 64);
        out.extend_from_slice(&self.sw_in);
        out.extend_from_slice(&self.sw_out);
        out.push(self.sub);
        out.push(self.acn_priority);
        out.push(self.command);
        out
    }
}

pub(crate) fn art_input(bind_index: u8, disabled: [bool; 4]) -> Vec<u8> {
    let mut out = header(op::INPUT);
    out.push(0);
    out.push(bind_index);
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend(disabled.iter().map(|&d| d as u8));
    out
}

pub(crate) fn art_ip_prog(command: u8, ip: [u8; 4], mask: [u8; 4], gateway: [u8; 4]) -> Vec<u8> {
    let mut out = header(op::IP_PROG);
    out.extend_from_slice(&[0, 0]);
    out.push(command);
    out.push(0);
    out.extend_from_slice(&ip);
    out.extend_from_slice(&mask);
    out.extend_from_slice(&[0, 0]); // ProgPort, deprecated
    out.extend_from_slice(&gateway);
    out.extend_from_slice(&[0; 4]);
    out
}

pub(crate) fn art_data_request(esta: u16, oem: u16, request: u16) -> Vec<u8> {
    let mut out = header(op::DATA_REQUEST);
    out.extend_from_slice(&esta.to_be_bytes());
    out.extend_from_slice(&oem.to_be_bytes());
    out.extend_from_slice(&request.to_be_bytes());
    out.extend_from_slice(&[0; 22]);
    out
}

pub(crate) fn art_command(esta: u16, text: &str) -> Vec<u8> {
    let mut out = header(op::COMMAND);
    out.extend_from_slice(&esta.to_be_bytes());
    let mut data = text.as_bytes().to_vec();
    data.push(0);
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(&data);
    out
}

pub(crate) fn art_trigger(oem: u16, key: u8, sub_key: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = header(op::TRIGGER);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&oem.to_be_bytes());
    out.push(key);
    out.push(sub_key);
    let mut data = payload.to_vec();
    data.resize(512, 0);
    out.extend_from_slice(&data);
    out
}

pub(crate) fn art_time_code(
    stream: u8,
    frames: u8,
    seconds: u8,
    minutes: u8,
    hours: u8,
    kind: u8,
) -> Vec<u8> {
    let mut out = header(op::TIME_CODE);
    out.extend_from_slice(&[0, stream, frames, seconds, minutes, hours, kind]);
    out
}

pub(crate) fn art_tod_request(universe: u16) -> Vec<u8> {
    let mut out = header(op::TOD_REQUEST);
    out.extend_from_slice(&[0; 9]); // Filler1-2, Spare1-7
    out.push((universe >> 8) as u8 & 0x7f);
    out.push(0); // TodFull
    out.push(1);
    let mut addresses = [0u8; 32];
    addresses[0] = (universe & 0xff) as u8;
    out.extend_from_slice(&addresses);
    out
}

pub(crate) fn art_tod_control(universe: u16, command: u8) -> Vec<u8> {
    let mut out = header(op::TOD_CONTROL);
    out.extend_from_slice(&[0; 9]);
    out.push((universe >> 8) as u8 & 0x7f);
    out.push(command);
    out.push((universe & 0xff) as u8);
    out
}

pub(crate) fn art_rdm(universe: u16, packet: &[u8]) -> Vec<u8> {
    let mut out = header(op::RDM);
    out.push(1); // RDM STANDARD V1.0
    out.extend_from_slice(&[0; 8]); // Filler2, Spare1-5, FifoAvail, FifoMax
    out.push((universe >> 8) as u8 & 0x7f);
    out.push(0); // ArProcess
    out.push((universe & 0xff) as u8);
    out.extend_from_slice(packet);
    out
}

/// The core's own ArtPollReply: a controller with no DMX ports.
pub(crate) fn own_poll_reply(me: &Identity, ip: Ipv4Addr, counter: u32) -> Vec<u8> {
    let mut out = ID.to_vec();
    out.extend_from_slice(&op::POLL_REPLY.to_le_bytes());
    out.extend_from_slice(&ip.octets());
    out.extend_from_slice(&PORT.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]); // VersInfo, NetSwitch, SubSwitch
    out.extend_from_slice(&me.oem.to_be_bytes());
    out.push(0); // UBEA
    out.push(0b1110_0000); // indicators normal, programming authority unknown
    out.extend_from_slice(&me.esta.to_le_bytes());
    text_field(&mut out, &me.short_name, 18);
    text_field(&mut out, &me.long_name, 64);
    text_field(
        &mut out,
        &format!("#0001 [{:04}] Power On Tests successful", counter % 10000),
        64,
    );
    out.extend_from_slice(&[0, 0]); // NumPorts
    out.extend_from_slice(&[0; 4 * 5]); // PortTypes, GoodInput, GoodOutputA, SwIn, SwOut
    out.extend_from_slice(&[100, 0, 0, 0, 0, 0]); // AcnPriority, SwMacro, SwRemote, Spare
    out.push(0x01); // StController
    out.extend_from_slice(&[0; 6]); // MAC
    out.extend_from_slice(&ip.octets()); // BindIp
    out.push(1); // BindIndex
    out.push(0b0000_1000); // Status2: 15-bit Port-Address
    out.extend_from_slice(&[0; 4]); // GoodOutputB
    out.push(0); // Status3
    out.extend_from_slice(&[0; 6]); // default responder UID
    out.extend_from_slice(&[0, 0, 0, 0, 0]); // User, RefreshRate, BackgroundQueuePolicy
    out.extend_from_slice(&[0; 10]);
    out
}

/// What the core says of itself.
#[derive(Debug, Clone)]
pub(crate) struct Identity {
    pub esta: u16,
    pub oem: u16,
    pub short_name: String,
    pub long_name: String,
}

/// One ArtPollReply, decoded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PollReply {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub bind_index: u8,
    pub num_ports: usize,
    pub net: u8,
    pub sub: u8,
    pub sw_in: [u8; 4],
    pub sw_out: [u8; 4],
    pub good_input: [u8; 4],
    pub state: Value,
}

impl PollReply {
    pub(crate) fn universe(&self, sw: u8) -> u16 {
        ((self.net as u16 & 0x7f) << 8) | ((self.sub as u16 & 0x0f) << 4) | (sw as u16 & 0x0f)
    }

    /// The universes the node subscribes to: every one its ports list in
    /// SwIn or SwOut (ArtDmx, "Unicast Subscription").
    pub(crate) fn subscribed(&self) -> BTreeSet<u16> {
        (0..self.num_ports.min(4))
            .flat_map(|i| [self.universe(self.sw_in[i]), self.universe(self.sw_out[i])])
            .collect()
    }
}

fn bit(byte: u8, n: u8) -> bool {
    byte & (1 << n) != 0
}

const STYLES: [&str; 7] = [
    "node",
    "controller",
    "media",
    "route",
    "backup",
    "config",
    "visual",
];
const PORT_TYPES: [&str; 7] = ["dmx", "midi", "avab", "cmx", "adb", "artnet", "dali"];

pub(crate) fn parse_poll_reply(p: &[u8]) -> Option<PollReply> {
    if p.len() < 207 {
        return None;
    }
    let mut buf = p.to_vec();
    buf.resize(239, 0); // missing fields are zero
    let p = &buf[..];
    let ip = Ipv4Addr::new(p[10], p[11], p[12], p[13]);
    let port = u16::from_le_bytes([p[14], p[15]]);
    let (net, sub) = (p[18] & 0x7f, p[19] & 0x0f);
    let status1 = p[23];
    let report = read_text(&p[108..172]);
    let (code, counter, report_text) = parse_report(&report);
    let num_ports = (u16::from_be_bytes([p[172], p[173]]) as usize).min(4);
    let types = &p[174..178];
    let good_input = [p[178], p[179], p[180], p[181]];
    let good_a = &p[182..186];
    let sw_in = [p[186], p[187], p[188], p[189]];
    let sw_out = [p[190], p[191], p[192], p[193]];
    let style = p[200];
    let bind_ip = Ipv4Addr::new(p[207], p[208], p[209], p[210]);
    let bind_index = p[211].max(1);
    let status2 = p[212];
    let good_b = &p[213..217];
    let status3 = p[217];
    let refresh = u16::from_be_bytes([p[226], p[227]]);
    let universe = |sw: u8| ((net as u16) << 8) | ((sub as u16) << 4) | (sw as u16 & 0x0f);
    let mut ports = Map::new();
    for i in 0..num_ports {
        let t = types[i];
        let kind = PORT_TYPES
            .get((t & 0x3f) as usize)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}", t & 0x3f));
        let gi = good_input[i];
        let ga = good_a[i];
        let gb = good_b[i];
        ports.insert(
            (i + 1).to_string(),
            json!({
                "type": kind,
                "can_output": bit(t, 7),
                "can_input": bit(t, 6),
                "input_universe": universe(sw_in[i]),
                "output_universe": universe(sw_out[i]),
                "input": {
                    "data_received": bit(gi, 7), "test_packets": bit(gi, 6), "sip": bit(gi, 5),
                    "text_packets": bit(gi, 4), "disabled": bit(gi, 3), "errors": bit(gi, 2),
                    "sacn": bit(gi, 0),
                },
                "output": {
                    "sending": bit(ga, 7), "test_packets": bit(ga, 6), "sip": bit(ga, 5),
                    "text_packets": bit(ga, 4), "merging": bit(ga, 3), "short": bit(ga, 2),
                    "ltp": bit(ga, 1), "sacn": bit(ga, 0),
                    "rdm_disabled": bit(gb, 7), "continuous": bit(gb, 6),
                    "discovery_idle": bit(gb, 5), "background_discovery_disabled": bit(gb, 4),
                },
            }),
        );
    }
    let indicators = ["unknown", "locate", "mute", "normal"][(status1 >> 6) as usize];
    let authority = ["unknown", "front_panel", "network", "unknown"][((status1 >> 4) & 3) as usize];
    let failsafe = ["hold", "zero", "full", "scene"][(status3 >> 6) as usize];
    let mac = p[201..207]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let style_name = STYLES
        .get(style as usize)
        .map(|s| s.to_string())
        .unwrap_or_else(|| style.to_string());
    let bits = |byte: u8| (0..8).map(|n| bit(byte, n)).collect::<Vec<_>>();
    let state = json!({
        "online": true,
        "ip": ip.to_string(),
        "bind_index": bind_index,
        "bind_ip": bind_ip.to_string(),
        "short_name": read_text(&p[26..44]),
        "long_name": read_text(&p[44..108]),
        "report": {"code": code, "counter": counter, "text": report_text},
        "firmware": u16::from_be_bytes([p[16], p[17]]),
        "net": net,
        "sub_net": sub,
        "oem": u16::from_be_bytes([p[20], p[21]]),
        "esta": u16::from_le_bytes([p[24], p[25]]),
        "ubea_version": p[22],
        "ubea_present": bit(status1, 0),
        "rdm_capable": bit(status1, 1),
        "rom_boot": bit(status1, 2),
        "indicators": indicators,
        "programming_authority": authority,
        "port_count": num_ports,
        "ports": ports,
        "acn_priority": p[194],
        "macros": bits(p[195]),
        "remotes": bits(p[196]),
    });
    let more = json!({
        "style": style_name,
        "mac": mac,
        "web_config": bit(status2, 0),
        "dhcp": bit(status2, 1),
        "dhcp_capable": bit(status2, 2),
        "port_address_15bit": bit(status2, 3),
        "sacn_switchable": bit(status2, 4),
        "squawking": bit(status2, 5),
        "output_style_switchable": bit(status2, 6),
        "rdm_control": bit(status2, 7),
        "failsafe": failsafe,
        "failsafe_programmable": bit(status3, 5),
        "llrp": bit(status3, 4),
        "port_direction_switchable": bit(status3, 3),
        "rdmnet": bit(status3, 2),
        "background_queue_supported": bit(status3, 1),
        "background_discovery_control": bit(status3, 0),
        "default_responder_uid": hex(&p[218..224]),
        "user": u16::from_be_bytes([p[224], p[225]]),
        "refresh_rate": if refresh <= 44 { 44 } else { refresh },
        "background_queue_policy": p[228],
    });
    let mut state = state;
    if let (Some(a), Value::Object(b)) = (state.as_object_mut(), more) {
        a.extend(b);
    }
    Some(PollReply {
        ip,
        port,
        bind_index,
        num_ports,
        net,
        sub,
        sw_in,
        sw_out,
        good_input,
        state,
    })
}

/// NodeReport, "#xxxx [yyyy] zzzz...": the hex status code, the decimal
/// counter and the text. A report in another shape is all text.
fn parse_report(report: &str) -> (i64, i64, String) {
    let parsed = (|| {
        let rest = report.strip_prefix('#')?;
        let (code, rest) = rest.split_once(' ')?;
        let code = i64::from_str_radix(code.trim(), 16).ok()?;
        let rest = rest.trim_start().strip_prefix('[')?;
        let (counter, text) = rest.split_once(']')?;
        let counter = counter.trim().parse().ok()?;
        Some((code, counter, text.trim().to_string()))
    })();
    parsed.unwrap_or((-1, -1, report.to_string()))
}

/// Which received universes go into state.
#[derive(Debug, Clone, PartialEq)]
enum Receive {
    None,
    All,
    Only(BTreeSet<u16>),
}

fn parse_receive(text: &str) -> Result<Receive, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Receive::None);
    }
    if text.eq_ignore_ascii_case("all") {
        return Ok(Receive::All);
    }
    let mut set = BTreeSet::new();
    for part in text.split(',') {
        let part = part.trim();
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let a: u16 = a
            .trim()
            .parse()
            .map_err(|_| format!("'{part}' is not a universe or range"))?;
        let b: u16 = b
            .trim()
            .parse()
            .map_err(|_| format!("'{part}' is not a universe or range"))?;
        if a > b || b > 32767 {
            return Err(format!("'{part}' is not a range of universes 0-32767"));
        }
        set.extend(a..=b);
    }
    Ok(Receive::Only(set))
}

fn parse_targets(text: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    text.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<SocketAddr>()
                .or_else(|_| t.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)))
                .map_err(|_| format!("poll address '{t}' is not an IP address or ip:port"))
        })
        .collect()
}

struct Node {
    addr: SocketAddr,
    subscribed: BTreeSet<u16>,
    inputs_disabled: [bool; 4],
    heard: bool,
    online: bool,
}

struct Universe {
    data: Vec<u8>,
    sequence: u8,
    dirty: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Wait {
    /// ArtAddress or ArtInput: the node answers with an ArtPollReply.
    PollReply(Ipv4Addr),
    IpProg(Ipv4Addr),
    Data(Ipv4Addr, u16),
    Rdm(Ipv4Addr, u16),
}

struct Pending {
    id: CommandId,
    wait: Wait,
    sent_at: Millis,
    deadline: Millis,
}

struct Received {
    data: Vec<u8>,
    source: SocketAddr,
    sequence: u8,
    reported_at: Option<Millis>,
    stale: bool,
}

pub(crate) struct ArtNet {
    port: u16,
    address: Option<IpAddr>,
    host: Option<IpAddr>,
    poll_targets: Vec<SocketAddr>,
    identity: Identity,
    diagnostics: bool,
    diag_priority: u8,
    refresh_ms: Millis,
    sync: bool,
    receive: Receive,
    receive_interval: Millis,
    monitor: bool,
    listening: bool,
    retry_after: Millis,
    nodes: BTreeMap<(Ipv4Addr, u8), Node>,
    universes: BTreeMap<u16, Universe>,
    last_flush: Option<Millis>,
    flush_armed: bool,
    pending: Vec<Pending>,
    received: BTreeMap<u16, Received>,
    /// Packets this side broadcast lately, to drop when heard back.
    sent: VecDeque<(Millis, Vec<u8>)>,
    reply_counter: u32,
    tod: BTreeMap<u16, Vec<String>>,
    nzs_sequence: u8,
}

fn setting_str<'a>(s: &'a Params, k: &str) -> Option<&'a str> {
    s.get(k).and_then(Value::as_str)
}

fn setting_int(s: &Params, k: &str) -> Option<i64> {
    match s.get(k) {
        Some(Value::String(t)) => t.trim().parse().ok(),
        Some(v) => v.as_i64(),
        None => None,
    }
}

impl ArtNet {
    pub(crate) fn new(ctx: OpenContext) -> Result<ArtNet, String> {
        let s = &ctx.settings;
        let port = ctx.port.unwrap_or(PORT);
        let address = match setting_str(s, "bind_address") {
            None | Some("") => None,
            Some(a) => Some(
                a.trim()
                    .parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let poll = setting_str(s, "poll_addresses").unwrap_or("2.255.255.255, 10.255.255.255");
        let mut poll_targets = parse_targets(poll, port)?;
        let host = (!ctx.host.is_unspecified()).then_some(ctx.host);
        if let Some(h) = host {
            let target = SocketAddr::new(h, port);
            if !poll_targets.contains(&target) {
                poll_targets.push(target);
            }
        }
        let identity = Identity {
            esta: setting_int(s, "esta_code").unwrap_or(0) as u16,
            oem: setting_int(s, "oem_code").unwrap_or(0x00ff) as u16,
            short_name: setting_str(s, "short_name").unwrap_or("Meros").to_string(),
            long_name: setting_str(s, "long_name")
                .unwrap_or("Meros Integrations")
                .to_string(),
        };
        let diag_priority = match setting_str(s, "diagnostics_priority").unwrap_or("low") {
            "low" => 0x10,
            "medium" => 0x40,
            "high" => 0x80,
            "critical" => 0xe0,
            "volatile" => 0xf0,
            other => return Err(format!("unknown diagnostics_priority '{other}'")),
        };
        Ok(ArtNet {
            port,
            address,
            host,
            poll_targets,
            identity,
            diagnostics: s
                .get("diagnostics")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            diag_priority,
            refresh_ms: setting_int(s, "refresh_ms").unwrap_or(1000).max(0) as Millis,
            sync: s.get("sync").and_then(Value::as_bool).unwrap_or(false),
            receive: parse_receive(setting_str(s, "receive").unwrap_or(""))?,
            receive_interval: setting_int(s, "receive_interval_ms").unwrap_or(100).max(0) as Millis,
            monitor: ctx.monitor,
            listening: false,
            retry_after: RETRY_MIN,
            nodes: BTreeMap::new(),
            universes: BTreeMap::new(),
            last_flush: None,
            flush_armed: false,
            pending: Vec::new(),
            received: BTreeMap::new(),
            sent: VecDeque::new(),
            reply_counter: 0,
            tod: BTreeMap::new(),
            nzs_sequence: 0,
        })
    }

    fn own_ip(&self) -> Ipv4Addr {
        match self.address {
            Some(IpAddr::V4(a)) => a,
            _ => Ipv4Addr::new(0, 0, 0, 0),
        }
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        cx.connection(Connection::Connected);
        cx.udp_open(
            SOCKET,
            Bind::Network {
                address: self.address,
                port: self.port,
            },
        );
        self.poll(cx);
        cx.set_timer(POLL, POLL_EVERY);
        if self.refresh_ms > 0 {
            cx.set_timer(REFRESH, self.refresh_ms);
        }
    }

    /// Send to each of the poll addresses, remembering it so that hearing it
    /// back is not taken for another device's.
    fn broadcast(&mut self, cx: &mut Cx, packet: Vec<u8>) {
        for to in self.poll_targets.clone() {
            cx.udp_send(SOCKET, to, packet.clone());
        }
        self.remember(cx.now(), packet);
    }

    fn remember(&mut self, now: Millis, packet: Vec<u8>) {
        while self
            .sent
            .front()
            .is_some_and(|(at, _)| now.saturating_sub(*at) > ECHO)
        {
            self.sent.pop_front();
        }
        if self.sent.len() >= 64 {
            self.sent.pop_front();
        }
        self.sent.push_back((now, packet));
    }

    fn is_echo(&self, now: Millis, packet: &[u8]) -> bool {
        self.sent
            .iter()
            .any(|(at, p)| now.saturating_sub(*at) <= ECHO && p.as_slice() == packet)
    }

    fn poll(&mut self, cx: &mut Cx) {
        // Bit 1: an ArtPollReply whenever the node's conditions change.
        let mut flags = 0b0000_0010;
        if self.diagnostics {
            flags |= 0b0000_1100; // send diagnostics, unicast
        }
        let packet = art_poll(
            flags,
            self.diag_priority,
            self.identity.esta,
            self.identity.oem,
        );
        self.broadcast(cx, packet);
    }

    fn poll_tick(&mut self, cx: &mut Cx) {
        let mut patch = Map::new();
        for ((ip, bind), node) in self.nodes.iter_mut() {
            if !node.heard && node.online {
                node.online = false;
                patch.insert(node_key(*ip, *bind), json!({"online": false}));
            }
            node.heard = false;
        }
        if !patch.is_empty() {
            cx.state(json!({"nodes": patch}));
            self.publish_destinations(cx);
        }
        self.poll(cx);
        cx.set_timer(POLL, POLL_EVERY);
    }

    fn destinations(&self, universe: u16) -> BTreeSet<SocketAddr> {
        let mut out: BTreeSet<SocketAddr> = self
            .nodes
            .values()
            .filter(|n| n.online && n.subscribed.contains(&universe))
            .map(|n| n.addr)
            .collect();
        if let Some(h) = self.host {
            out.insert(SocketAddr::new(h, self.port));
        }
        out
    }

    fn publish_destinations(&self, cx: &mut Cx) {
        if self.universes.is_empty() {
            return;
        }
        let mut outputs = Map::new();
        for &u in self.universes.keys() {
            let list: Vec<String> = self.destinations(u).iter().map(|a| a.to_string()).collect();
            outputs.insert(u.to_string(), json!({"destinations": list}));
        }
        cx.state(json!({"outputs": outputs}));
    }

    /// Send the universes that changed, or every held one.
    fn send_universes(&mut self, cx: &mut Cx, all: bool) {
        let mut sent_any = false;
        let keys: Vec<u16> = self.universes.keys().copied().collect();
        for u in keys {
            let targets = self.destinations(u);
            let entry = self.universes.get_mut(&u).unwrap();
            if !(all || entry.dirty) {
                continue;
            }
            entry.dirty = false;
            if targets.is_empty() {
                continue;
            }
            entry.sequence = if entry.sequence == 255 {
                1
            } else {
                entry.sequence + 1
            };
            let packet = art_dmx(entry.sequence, u, &entry.data);
            for to in targets {
                cx.udp_send(SOCKET, to, packet.clone());
            }
            sent_any = true;
        }
        if sent_any && self.sync {
            self.broadcast(cx, art_sync());
        }
        self.last_flush = Some(cx.now());
    }

    /// A universe changed: send it now, or when 23 ms have passed since the
    /// last frame.
    fn changed(&mut self, cx: &mut Cx) {
        if self.flush_armed || !self.listening {
            return;
        }
        let since = self
            .last_flush
            .map(|at| cx.now().saturating_sub(at))
            .unwrap_or(FRAME_GAP);
        if since >= FRAME_GAP {
            self.send_universes(cx, false);
        } else {
            self.flush_armed = true;
            cx.set_timer(FLUSH, FRAME_GAP - since);
        }
    }

    fn universe_state(&self, cx: &mut Cx, u: u16) {
        let Some(entry) = self.universes.get(&u) else {
            return;
        };
        let list: Vec<String> = self.destinations(u).iter().map(|a| a.to_string()).collect();
        cx.state(json!({"outputs": {u.to_string(): {
            "channels": entry.data, "destinations": list,
        }}}));
    }

    fn node_addr(&self, ip: Ipv4Addr) -> SocketAddr {
        self.nodes
            .iter()
            .find(|((i, _), _)| *i == ip)
            .map(|(_, n)| n.addr)
            .unwrap_or(SocketAddr::new(IpAddr::V4(ip), self.port))
    }

    fn wait(&mut self, cx: &mut Cx, id: CommandId, wait: Wait) {
        let now = cx.now();
        self.pending.push(Pending {
            id,
            wait,
            sent_at: now,
            deadline: now + ANSWER,
        });
        self.arm_deadline(cx);
    }

    fn arm_deadline(&self, cx: &mut Cx) {
        if let Some(first) = self.pending.iter().map(|p| p.deadline).min() {
            cx.set_timer(DEADLINE, first.saturating_sub(cx.now()));
        } else {
            cx.cancel_timer(DEADLINE);
        }
    }

    /// Complete every command waiting for this; `value` is its result.
    fn answered(&mut self, cx: &mut Cx, wait: &Wait, value: Option<Value>) {
        let now = cx.now();
        let mut done = Vec::new();
        self.pending.retain(|p| {
            if &p.wait == wait {
                done.push((p.id, p.sent_at));
                false
            } else {
                true
            }
        });
        for (id, sent_at) in done {
            cx.round_trip(now.saturating_sub(sent_at));
            let outcome = match &value {
                Some(v) => Outcome::Value { value: v.clone() },
                None => Outcome::Ack,
            };
            cx.complete(id, Ok(outcome));
        }
        self.arm_deadline(cx);
    }

    fn packet(&mut self, cx: &mut Cx, from: SocketAddr, p: &[u8]) {
        if p.len() < 10 || &p[..8] != ID {
            return;
        }
        let now = cx.now();
        let opcode = u16::from_le_bytes([p[8], p[9]]);
        // The core answers its own poll, and drops its own reply when it hears
        // that (poll_reply); anything else of its own heard back is dropped.
        if opcode != op::POLL_REPLY && opcode != op::POLL && self.is_echo(now, p) {
            return;
        }
        cx.alive();
        let ip = match from.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(ip) => ip,
                None => return,
            },
        };
        match opcode {
            op::POLL => {
                self.reply_counter = self.reply_counter.wrapping_add(1);
                let reply = own_poll_reply(&self.identity, self.own_ip(), self.reply_counter);
                cx.udp_send(SOCKET, from, reply.clone());
                self.remember(now, reply);
            }
            op::POLL_REPLY => self.poll_reply(cx, from, ip, p),
            op::DMX if p.len() >= 18 => self.dmx(cx, from, p),
            op::NZS if p.len() >= 18 => {
                let len = (u16::from_be_bytes([p[16], p[17]]) as usize).min(p.len() - 18);
                let universe = u16::from_le_bytes([p[14], p[15] & 0x7f]);
                cx.message(
                    "ArtNzs",
                    None,
                    json!({"universe": universe, "start_code": p[13], "sequence": p[12],
                           "data": hex(&p[18..18 + len])}),
                    from,
                );
            }
            op::SYNC => cx.message("ArtSync", None, json!({}), from),
            op::DIAG_DATA if p.len() >= 18 => {
                let len = (u16::from_be_bytes([p[16], p[17]]) as usize).min(p.len() - 18);
                let text = read_text(&p[18..18 + len]);
                let priority = match p[13] {
                    0x10 => "low".to_string(),
                    0x40 => "medium".to_string(),
                    0x80 => "high".to_string(),
                    0xe0 => "critical".to_string(),
                    0xf0 => "volatile".to_string(),
                    n => n.to_string(),
                };
                cx.state(json!({"diagnostics": {ip.to_string(): {
                    "text": text, "priority": priority, "port": p[14],
                }}}));
                cx.message(
                    "ArtDiagData",
                    None,
                    json!({"priority": priority, "port": p[14], "text": text}),
                    from,
                );
            }
            op::TIME_CODE if p.len() >= 19 => {
                let kind = ["film", "ebu", "df", "smpte"]
                    .get(p[18] as usize)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| p[18].to_string());
                let tc = json!({
                    "hours": p[17], "minutes": p[16], "seconds": p[15], "frames": p[14],
                    "type": kind, "stream": p[13], "source": ip.to_string(),
                });
                cx.state(json!({"timecode": tc.clone()}));
                cx.message("ArtTimeCode", None, tc, from);
            }
            op::COMMAND if p.len() >= 16 => {
                let len = (u16::from_be_bytes([p[14], p[15]]) as usize).min(p.len() - 16);
                cx.message(
                    "ArtCommand",
                    None,
                    json!({"esta": u16::from_be_bytes([p[12], p[13]]),
                           "text": read_text(&p[16..16 + len])}),
                    from,
                );
            }
            op::TRIGGER if p.len() >= 18 => {
                let payload = &p[18..p.len().min(530)];
                let used = payload.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                cx.message(
                    "ArtTrigger",
                    None,
                    json!({"oem": u16::from_be_bytes([p[14], p[15]]), "key": p[16],
                           "sub_key": p[17], "payload": hex(&payload[..used])}),
                    from,
                );
            }
            op::IP_PROG_REPLY if p.len() >= 32 => {
                let config = json!({
                    "ip": Ipv4Addr::new(p[16], p[17], p[18], p[19]).to_string(),
                    "subnet_mask": Ipv4Addr::new(p[20], p[21], p[22], p[23]).to_string(),
                    "gateway": Ipv4Addr::new(p[28], p[29], p[30], p[31]).to_string(),
                    "dhcp": bit(p[26], 6),
                });
                let mut patch = Map::new();
                for (ni, bind) in self.nodes.keys().filter(|(i, _)| *i == ip) {
                    patch.insert(node_key(*ni, *bind), json!({"ip_config": config.clone()}));
                }
                if !patch.is_empty() {
                    cx.state(json!({"nodes": patch}));
                }
                self.answered(cx, &Wait::IpProg(ip), Some(config));
            }
            op::DATA_REPLY if p.len() >= 20 => {
                let request = u16::from_be_bytes([p[16], p[17]]);
                let len = (u16::from_be_bytes([p[18], p[19]]) as usize).min(p.len() - 20);
                let text = read_text(&p[20..20 + len]);
                if let Some(name) = data_name(request) {
                    let mut patch = Map::new();
                    for (ni, bind) in self.nodes.keys().filter(|(i, _)| *i == ip) {
                        patch.insert(node_key(*ni, *bind), json!({"data": {name: text.clone()}}));
                    }
                    if !patch.is_empty() {
                        cx.state(json!({"nodes": patch}));
                    }
                }
                self.answered(cx, &Wait::Data(ip, request), Some(Value::String(text)));
            }
            op::TOD_DATA if p.len() >= 28 => self.tod_data(cx, ip, p),
            op::RDM if p.len() >= 24 => {
                let universe = ((p[21] as u16 & 0x7f) << 8) | p[23] as u16;
                let packet = hex(&p[24..]);
                cx.message(
                    "ArtRdm",
                    None,
                    json!({"universe": universe, "packet": packet}),
                    from,
                );
                self.answered(cx, &Wait::Rdm(ip, universe), Some(Value::String(packet)));
            }
            _ => {}
        }
    }

    fn poll_reply(&mut self, cx: &mut Cx, from: SocketAddr, ip: Ipv4Addr, p: &[u8]) {
        // The core's own reply, heard back, is no node.
        if self.is_echo(cx.now(), p) {
            return;
        }
        let Some(reply) = parse_poll_reply(p) else {
            cx.log(Level::Debug, format!("short ArtPollReply from {from}"));
            return;
        };
        // Replies go to the node's address and the port it gives; the
        // specification fixes that port at 6454, so a node giving none is
        // reached there.
        let port = if reply.port == 0 {
            self.port
        } else {
            reply.port
        };
        let addr = SocketAddr::new(IpAddr::V4(ip), port);
        let key = (ip, reply.bind_index);
        let mut disabled = [false; 4];
        for (i, d) in disabled.iter_mut().enumerate() {
            *d = bit(reply.good_input[i], 3);
        }
        let subscribed = reply.subscribed();
        let changed_subscription = self
            .nodes
            .get(&key)
            .is_none_or(|n| n.subscribed != subscribed || !n.online || n.addr != addr);
        self.nodes.insert(
            key,
            Node {
                addr,
                subscribed,
                inputs_disabled: disabled,
                heard: true,
                online: true,
            },
        );
        let mut state = reply.state;
        state["port"] = json!(port);
        cx.state(json!({"nodes": {node_key(ip, reply.bind_index): state}}));
        if changed_subscription {
            self.publish_destinations(cx);
        }
        self.answered(cx, &Wait::PollReply(ip), None);
    }

    fn dmx(&mut self, cx: &mut Cx, from: SocketAddr, p: &[u8]) {
        if !self.monitor {
            return;
        }
        let universe = u16::from_le_bytes([p[14], p[15] & 0x7f]);
        let wanted = match &self.receive {
            Receive::None => false,
            Receive::All => true,
            Receive::Only(set) => set.contains(&universe),
        };
        if !wanted {
            return;
        }
        let len = (u16::from_be_bytes([p[16], p[17]]) as usize).min(p.len() - 18);
        let now = cx.now();
        let entry = self.received.entry(universe).or_insert(Received {
            data: Vec::new(),
            source: from,
            sequence: 0,
            reported_at: None,
            stale: false,
        });
        let changed = entry.data != p[18..18 + len] || entry.source != from;
        entry.data = p[18..18 + len].to_vec();
        entry.source = from;
        entry.sequence = p[12];
        if !changed {
            return;
        }
        let due = entry
            .reported_at
            .is_none_or(|at| now.saturating_sub(at) >= self.receive_interval);
        if due {
            entry.reported_at = Some(now);
            entry.stale = false;
            report_input(cx, universe, entry);
        } else if !entry.stale {
            entry.stale = true;
            let wait = self.receive_interval - now.saturating_sub(entry.reported_at.unwrap_or(now));
            cx.set_timer(input_timer(), wait.max(1));
        }
    }

    fn report_stale_inputs(&mut self, cx: &mut Cx) {
        let now = cx.now();
        for (u, entry) in self.received.iter_mut() {
            if entry.stale {
                entry.stale = false;
                entry.reported_at = Some(now);
                report_input(cx, *u, entry);
            }
        }
    }

    fn tod_data(&mut self, cx: &mut Cx, ip: Ipv4Addr, p: &[u8]) {
        let universe = ((p[21] as u16 & 0x7f) << 8) | p[23] as u16;
        if p[22] == 0xff {
            cx.state(json!({"rdm": {universe.to_string(): {"available": false, "gateway": ip.to_string()}}}));
            return;
        }
        let total = u16::from_be_bytes([p[24], p[25]]);
        let block = p[26];
        let count = p[27] as usize;
        let uids: Vec<String> = p[28..]
            .as_chunks::<6>()
            .0
            .iter()
            .take(count)
            .map(|uid| hex(uid))
            .collect();
        let list = self.tod.entry(universe).or_default();
        if block == 0 {
            list.clear();
        }
        list.extend(uids);
        cx.state(json!({"rdm": {universe.to_string(): {
            "available": true, "total": total, "uids": list.clone(), "gateway": ip.to_string(),
        }}}));
    }

    fn fail(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
        cx.complete(
            id,
            Err(CommandError::InvalidParams {
                message: message.into(),
            }),
        );
    }
}

fn report_input(cx: &mut Cx, universe: u16, entry: &Received) {
    cx.state(json!({"inputs": {universe.to_string(): {
        "channels": entry.data, "source": entry.source.ip().to_string(),
        "sequence": entry.sequence,
    }}}));
}

fn input_timer() -> Key {
    "inputs"
}

fn node_key(ip: Ipv4Addr, bind: u8) -> String {
    format!("{ip}/{bind}")
}

const DATA_REQUESTS: [(&str, u16); 6] = [
    ("poll", 0x0000),
    ("product_url", 0x0001),
    ("user_guide_url", 0x0002),
    ("support_url", 0x0003),
    ("personality_udr_url", 0x0004),
    ("personality_gdtf_url", 0x0005),
];

fn data_name(code: u16) -> Option<&'static str> {
    DATA_REQUESTS
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(n, _)| *n)
        .filter(|n| *n != "poll")
}

/// ArtAddress Command values by name; a `port` adds 0 to 3 to those that
/// name a port.
fn address_command(action: &str, port: u8) -> Option<u8> {
    let p = port.saturating_sub(1) & 3;
    Some(match action {
        "none" => 0x00,
        "cancel_merge" => 0x01,
        "led_normal" => 0x02,
        "led_mute" => 0x03,
        "led_locate" => 0x04,
        "reset_rx_flags" => 0x05,
        "analysis_on" => 0x06,
        "analysis_off" => 0x07,
        "fail_hold" => 0x08,
        "fail_zero" => 0x09,
        "fail_full" => 0x0a,
        "fail_scene" => 0x0b,
        "fail_record" => 0x0c,
        "merge_ltp" => 0x10 + p,
        "direction_output" => 0x20 + p,
        "direction_input" => 0x30 + p,
        "merge_htp" => 0x50 + p,
        "select_artnet" => 0x60 + p,
        "select_sacn" => 0x70 + p,
        "clear_output" => 0x90 + p,
        "style_delta" => 0xa0 + p,
        "style_constant" => 0xb0 + p,
        "rdm_enable" => 0xc0 + p,
        "rdm_disable" => 0xd0 + p,
        _ => return None,
    })
}

fn byte_values(v: Option<&Value>, max: usize) -> Result<Vec<u8>, String> {
    let list = v
        .and_then(Value::as_array)
        .ok_or("values must be a list of integers 0-255")?;
    if list.len() > max {
        return Err(format!("at most {max} values"));
    }
    list.iter()
        .map(|x| {
            x.as_u64()
                .filter(|n| *n <= 255)
                .map(|n| n as u8)
                .ok_or_else(|| format!("{x} is not an integer 0-255"))
        })
        .collect()
}

fn ipv4(text: &str) -> Result<Ipv4Addr, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("'{text}' is not an IPv4 address"))
}

impl Module for ArtNet {
    fn start(&mut self, cx: &mut Cx) {
        let targets: Vec<String> = self.poll_targets.iter().map(|a| a.to_string()).collect();
        cx.state(json!({"port": self.port, "poll_addresses": targets}));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let int = |k: &str| params.get(k).and_then(Value::as_i64).unwrap_or(0);
        let text = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        if !self.listening {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let universe = int("universe") as u16;
        let bind_index = (int("bind_index").max(1)) as u8;
        // Commands to one node: its address, from its ArtPollReply.
        let node = match params.get("node").and_then(Value::as_str) {
            Some(n) => match ipv4(n) {
                Ok(ip) => Some(ip),
                Err(e) => return Self::fail(cx, id, e),
            },
            None => None,
        };
        // An optional target: one address, or the poll addresses.
        let target = text("target");
        let targets: Vec<SocketAddr> = if target.trim().is_empty() {
            self.poll_targets.clone()
        } else {
            match parse_targets(&target, self.port) {
                Ok(t) => t,
                Err(e) => return Self::fail(cx, id, e.replace("poll address", "target")),
            }
        };
        let send_to = |me: &mut ArtNet, cx: &mut Cx, packet: Vec<u8>| {
            for to in &targets {
                cx.udp_send(SOCKET, *to, packet.clone());
            }
            me.remember(cx.now(), packet);
        };
        match name {
            "set_channel" | "set_channels" | "set_universe" | "clear_universe" => {
                let values = match name {
                    "set_channel" => vec![int("value") as u8],
                    "clear_universe" => vec![0; 512],
                    _ => {
                        let max = if name == "set_universe" {
                            512
                        } else {
                            513 - int("start").max(1) as usize
                        };
                        match byte_values(params.get("values"), max) {
                            Ok(v) => v,
                            Err(e) => return Self::fail(cx, id, e),
                        }
                    }
                };
                let start = match name {
                    "set_channel" => int("channel") as usize,
                    "set_channels" => int("start") as usize,
                    _ => 1,
                };
                let entry = self.universes.entry(universe).or_insert(Universe {
                    data: vec![0; 512],
                    sequence: 0,
                    dirty: true,
                });
                if name == "set_universe" {
                    entry.data = vec![0; 512];
                }
                for (i, v) in values.iter().enumerate() {
                    entry.data[start - 1 + i] = *v;
                }
                entry.dirty = true;
                self.universe_state(cx, universe);
                self.changed(cx);
                let sending = !self.destinations(universe).is_empty();
                cx.complete(
                    id,
                    Ok(if sending {
                        Outcome::Unverified
                    } else {
                        Outcome::Value {
                            value: json!({"held": true, "destinations": 0}),
                        }
                    }),
                );
            }
            "release_universe" => {
                if self.universes.remove(&universe).is_some() {
                    cx.state(json!({"outputs": {universe.to_string(): Value::Null}}));
                }
                cx.complete(id, Ok(Outcome::Ack));
            }
            "release_all" => {
                if !self.universes.is_empty() {
                    let gone: Map<String, Value> = self
                        .universes
                        .keys()
                        .map(|u| (u.to_string(), Value::Null))
                        .collect();
                    cx.state(json!({"outputs": gone}));
                    self.universes.clear();
                }
                cx.complete(id, Ok(Outcome::Ack));
            }
            "send_sync" => {
                self.broadcast(cx, art_sync());
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "poll" => {
                self.poll(cx);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_nzs" => {
                let values = match byte_values(params.get("values"), 512) {
                    Ok(v) if !v.is_empty() => v,
                    Ok(_) => return Self::fail(cx, id, "values needs at least one value"),
                    Err(e) => return Self::fail(cx, id, e),
                };
                let start_code = int("start_code") as u8;
                if start_code == 0 || start_code == 0xcc {
                    return Self::fail(
                        cx,
                        id,
                        "start_code may be neither 0 (ArtDmx) nor 0xCC (RDM)",
                    );
                }
                self.nzs_sequence = if self.nzs_sequence == 255 {
                    1
                } else {
                    self.nzs_sequence + 1
                };
                let packet = art_nzs(self.nzs_sequence, start_code, universe, &values);
                let dest = self.destinations(universe);
                for to in dest {
                    cx.udp_send(SOCKET, to, packet.clone());
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_trigger" => {
                let payload = match unhex(&text("payload")) {
                    Some(p) if p.len() <= 512 => p,
                    _ => return Self::fail(cx, id, "payload is not up to 512 bytes of hex"),
                };
                let packet = art_trigger(
                    int("oem") as u16,
                    int("key") as u8,
                    int("sub_key") as u8,
                    &payload,
                );
                send_to(self, cx, packet);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_command" => {
                let packet = art_command(int("esta") as u16, &text("text"));
                send_to(self, cx, packet);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_timecode" => {
                let kind = match text("type").as_str() {
                    "film" => 0,
                    "ebu" => 1,
                    "df" => 2,
                    _ => 3,
                };
                let max_frame = [23, 24, 29, 29][kind as usize];
                if int("frames") > max_frame {
                    return Self::fail(cx, id, format!("frames is 0-{max_frame} at this rate"));
                }
                let packet = art_time_code(
                    int("stream") as u8,
                    int("frames") as u8,
                    int("seconds") as u8,
                    int("minutes") as u8,
                    int("hours") as u8,
                    kind,
                );
                send_to(self, cx, packet);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "request_rdm_tod" => {
                let packet = art_tod_request(universe);
                match node {
                    Some(ip) => cx.udp_send(SOCKET, self.node_addr(ip), packet),
                    None => self.broadcast(cx, packet),
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "rdm_tod_control" => {
                let command = match text("action").as_str() {
                    "flush" => 1,
                    "end" => 2,
                    "incremental_on" => 3,
                    "incremental_off" => 4,
                    _ => 0,
                };
                let packet = art_tod_control(universe, command);
                match node {
                    Some(ip) => cx.udp_send(SOCKET, self.node_addr(ip), packet),
                    None => self.broadcast(cx, packet),
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_rdm" => {
                let Some(ip) = node else {
                    return Self::fail(cx, id, "node is required");
                };
                let packet = match unhex(&text("packet")) {
                    Some(p) if !p.is_empty() && p.len() <= 512 => p,
                    _ => return Self::fail(cx, id, "packet is not 1 to 512 bytes of hex"),
                };
                cx.udp_send(SOCKET, self.node_addr(ip), art_rdm(universe, &packet));
                self.wait(cx, id, Wait::Rdm(ip, universe));
            }
            // Node configuration: every one of these needs the node.
            _ => {
                let Some(ip) = node else {
                    return Self::fail(cx, id, "node is required");
                };
                let to = self.node_addr(ip);
                let mut address = Address::unchanged(bind_index);
                let packet = match name {
                    "set_node_names" => {
                        address.short_name = text("short_name");
                        address.long_name = text("long_name");
                        address.encode()
                    }
                    "set_node_port_address" => {
                        let port = (int("port").clamp(1, 4) - 1) as usize;
                        address.net = 0x80 | ((universe >> 8) as u8 & 0x7f);
                        address.sub = 0x80 | ((universe >> 4) as u8 & 0x0f);
                        let sw = 0x80 | (universe as u8 & 0x0f);
                        if text("direction") == "input" {
                            address.sw_in[port] = sw;
                        } else {
                            address.sw_out[port] = sw;
                        }
                        address.encode()
                    }
                    "reset_node_port_addresses" => {
                        address.net = 0;
                        address.sub = 0;
                        address.sw_in = [0; 4];
                        address.sw_out = [0; 4];
                        address.encode()
                    }
                    "set_node_acn_priority" => {
                        address.acn_priority = int("priority") as u8;
                        address.encode()
                    }
                    "node_command" => {
                        let Some(command) = address_command(&text("action"), int("port") as u8)
                        else {
                            return Self::fail(
                                cx,
                                id,
                                format!("unknown action '{}'", text("action")),
                            );
                        };
                        address.command = command;
                        address.encode()
                    }
                    "set_node_background_queue_policy" => {
                        address.command = 0xe0 + (int("policy") as u8 & 0x0f);
                        address.encode()
                    }
                    "set_node_input_enabled" => {
                        let port = (int("port").clamp(1, 4) - 1) as usize;
                        let mut disabled = self
                            .nodes
                            .get(&(ip, bind_index))
                            .map(|n| n.inputs_disabled)
                            .unwrap_or([false; 4]);
                        disabled[port] = !params
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                        art_input(bind_index, disabled)
                    }
                    "get_node_ip_config" | "set_node_ip" | "set_node_dhcp" | "reset_node_ip" => {
                        let (command, addrs) = match name {
                            "get_node_ip_config" => (0, None),
                            "set_node_dhcp" => (0x80 | 0x40, None),
                            "reset_node_ip" => (0x80 | 0x08, None),
                            _ => {
                                let parse = |k: &str| ipv4(&text(k)).map(|a| a.octets());
                                match (parse("ip"), parse("subnet_mask"), parse("gateway")) {
                                    (Ok(a), Ok(m), Ok(g)) => {
                                        (0x80 | 0x10 | 0x04 | 0x02, Some((a, m, g)))
                                    }
                                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                                        return Self::fail(cx, id, e)
                                    }
                                }
                            }
                        };
                        let (a, m, g) = addrs.unwrap_or(([0; 4], [0; 4], [0; 4]));
                        cx.udp_send(SOCKET, to, art_ip_prog(command, a, m, g));
                        self.wait(cx, id, Wait::IpProg(ip));
                        return;
                    }
                    "get_node_data" => {
                        let request = DATA_REQUESTS
                            .iter()
                            .find(|(n, _)| *n == text("request"))
                            .map(|(_, c)| *c)
                            .unwrap_or(0);
                        cx.udp_send(
                            SOCKET,
                            to,
                            art_data_request(self.identity.esta, self.identity.oem, request),
                        );
                        self.wait(cx, id, Wait::Data(ip, request));
                        return;
                    }
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
                cx.udp_send(SOCKET, to, packet);
                self.wait(cx, id, Wait::PollReply(ip));
            }
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        self.packet(cx, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.listening = false;
        cx.log(Level::Warning, format!("Art-Net port: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        cx.cancel_timer(POLL);
        cx.cancel_timer(REFRESH);
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.listen(cx),
            POLL if self.listening => self.poll_tick(cx),
            REFRESH if self.listening => {
                self.send_universes(cx, true);
                cx.set_timer(REFRESH, self.refresh_ms);
            }
            FLUSH => {
                self.flush_armed = false;
                if self.listening {
                    self.send_universes(cx, false);
                }
            }
            DEADLINE => {
                let now = cx.now();
                let mut late = Vec::new();
                self.pending.retain(|p| {
                    if p.deadline <= now {
                        late.push(p.id);
                        false
                    } else {
                        true
                    }
                });
                for id in late {
                    cx.complete(id, Err(CommandError::Timeout));
                }
                self.arm_deadline(cx);
            }
            k if k == input_timer() => self.report_stale_inputs(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;

    fn module(settings: Value, host: Option<Ipv4Addr>, monitor: bool) -> ArtNet {
        ArtNet::new(OpenContext {
            host: IpAddr::V4(host.unwrap_or(Ipv4Addr::new(0, 0, 0, 0))),
            host_name: None,
            port: None,
            model: "artnet".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
        .unwrap()
    }

    fn sends(actions: &[Action]) -> Vec<(SocketAddr, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { to, data, .. } => Some((*to, data.clone())),
                _ => None,
            })
            .collect()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// An ArtPollReply as a node with one output port on universe 0x0123
    /// (Net 1, Sub-Net 2, SwOut 3) would send.
    fn node_reply(ip: [u8; 4]) -> Vec<u8> {
        let mut p = vec![0u8; 239];
        p[..8].copy_from_slice(b"Art-Net\0");
        p[8..10].copy_from_slice(&0x2100u16.to_le_bytes());
        p[10..14].copy_from_slice(&ip);
        p[14..16].copy_from_slice(&0x1936u16.to_le_bytes());
        p[16] = 1;
        p[17] = 2;
        p[18] = 1;
        p[19] = 2;
        p[20] = 0x12;
        p[21] = 0x34;
        p[23] = 0b1110_0010;
        p[24] = 0x41; // ESTA "AL", low byte first
        p[25] = 0x4c;
        p[26..30].copy_from_slice(b"Node");
        p[44..53].copy_from_slice(b"Long node");
        p[108..133].copy_from_slice(b"#0001 [0042] Power On Ok.");
        p[173] = 1;
        p[174] = 0x80;
        p[182] = 0x80;
        p[190] = 3;
        p[194] = 100;
        p[200] = 0;
        p[201..207].copy_from_slice(&[0x12, 0x45, 0x78, 0x98, 0x34, 0x76]);
        p[211] = 1;
        p[212] = 0b0000_1101;
        p[217] = 0b0110_0000;
        p
    }

    #[test]
    fn polls_the_default_addresses_every_three_seconds() {
        let mut m = module(json!({}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Network {
                address: None,
                port: 6454
            }
        }));
        let s = sends(&a);
        let poll = hex(&s[0].1);
        // ID, OpPoll low byte first, version 14, flags 0x02, priority 0x10,
        // no target range, ESTA 0, OEM 0x00ff.
        assert_eq!(poll, "4172742d4e6574000020000e021000000000000000ff");
        assert_eq!(
            s.iter().map(|(to, _)| to.to_string()).collect::<Vec<_>>(),
            ["2.255.255.255:6454", "10.255.255.255:6454"]
        );
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: 3000
        }));
    }

    #[test]
    fn a_poll_reply_is_kept_and_a_silent_node_goes_offline() {
        let mut m = module(json!({}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let from: SocketAddr = "2.1.2.3:6454".parse().unwrap();
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from, &node_reply([2, 1, 2, 3]));
        let a = cx.take();
        let patch = a
            .iter()
            .find_map(|a| match a {
                Action::State(p) if p.get("nodes").is_some() => Some(p.clone()),
                _ => None,
            })
            .unwrap();
        let n = &patch["nodes"]["2.1.2.3/1"];
        assert_eq!(n["short_name"], "Node");
        assert_eq!(n["long_name"], "Long node");
        assert_eq!(
            n["report"],
            json!({"code": 1, "counter": 42, "text": "Power On Ok."})
        );
        assert_eq!(n["firmware"], 0x0102);
        assert_eq!(n["oem"], 0x1234);
        assert_eq!(n["esta"], 0x4c41);
        assert_eq!(n["indicators"], "normal");
        assert_eq!(n["rdm_capable"], true);
        assert_eq!(n["ports"]["1"]["output_universe"], 0x0123);
        assert_eq!(n["ports"]["1"]["output"]["sending"], true);
        assert_eq!(n["mac"], "12:45:78:98:34:76");
        assert_eq!(n["web_config"], true);
        assert_eq!(n["dhcp_capable"], true);
        assert_eq!(n["failsafe"], "zero");
        assert_eq!(n["failsafe_programmable"], true);
        assert_eq!(n["refresh_rate"], 44);
        // Polled again without a reply in between: offline.
        let mut cx = Cx::new(3000);
        m.timer(&mut cx, POLL);
        cx.take();
        let mut cx = Cx::new(6000);
        m.timer(&mut cx, POLL);
        assert!(cx.take().contains(&Action::State(
            json!({"nodes": {"2.1.2.3/1": {"online": false}}})
        )));
    }

    #[test]
    fn dmx_goes_unicast_to_subscribers_only() {
        let mut m = module(json!({"refresh_ms": 1000}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        // No subscriber yet: held, not sent.
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "set_channel",
            &params(json!({"universe": 0x0123, "channel": 2, "value": 255})),
        );
        let a = cx.take();
        assert!(sends(&a).is_empty());
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"held": true, "destinations": 0})
            })
        }));
        let from: SocketAddr = "2.1.2.3:6454".parse().unwrap();
        let mut cx = Cx::new(200);
        m.datagram(&mut cx, SOCKET, from, &node_reply([2, 1, 2, 3]));
        cx.take();
        let mut cx = Cx::new(300);
        m.command(
            &mut cx,
            2,
            "set_channels",
            &params(json!({"universe": 0x0123, "start": 1, "values": [10, 20]})),
        );
        let s = sends(&cx.take());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].0, from);
        let p = &s[0].1;
        // OpDmx, version 14, sequence 1, physical 0, SubUni 0x23, Net 0x01,
        // length 512 high byte first, then the data.
        assert_eq!(hex(&p[..18]), "4172742d4e6574000050000e010023010200");
        assert_eq!(&p[18..21], &[10, 20, 0]);
        assert_eq!(p.len(), 18 + 512);
        // A second change within 23 ms waits for the frame gap.
        let mut cx = Cx::new(310);
        m.command(
            &mut cx,
            3,
            "set_channel",
            &params(json!({"universe": 0x0123, "channel": 3, "value": 1})),
        );
        let a = cx.take();
        assert!(sends(&a).is_empty());
        assert!(a.contains(&Action::SetTimer {
            key: FLUSH,
            after: 13
        }));
        let mut cx = Cx::new(323);
        m.timer(&mut cx, FLUSH);
        let s = sends(&cx.take());
        assert_eq!(s[0].1[12], 2, "sequence");
        assert_eq!(&s[0].1[18..21], &[10, 20, 1]);
        // The refresh sends every held universe again.
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, REFRESH);
        let a = cx.take();
        assert_eq!(sends(&a).len(), 1);
        assert!(a.contains(&Action::SetTimer {
            key: REFRESH,
            after: 1000
        }));
    }

    #[test]
    fn sync_follows_each_burst() {
        let mut m = module(
            json!({"sync": true}),
            Some(Ipv4Addr::new(10, 0, 0, 5)),
            true,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "clear_universe",
            &params(json!({"universe": 1})),
        );
        let s = sends(&cx.take());
        assert_eq!(s[0].0, "10.0.0.5:6454".parse::<SocketAddr>().unwrap());
        assert_eq!(hex(&s[1].1), "4172742d4e6574000052000e0000");
        assert_eq!(
            s.len(),
            4,
            "the DMX, then ArtSync to both poll addresses and the host"
        );
    }

    #[test]
    fn node_configuration_completes_on_the_node_reply() {
        let mut m = module(json!({}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let from: SocketAddr = "2.1.2.3:6454".parse().unwrap();
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from, &node_reply([2, 1, 2, 3]));
        cx.take();
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            7,
            "set_node_port_address",
            &params(json!({"node": "2.1.2.3", "bind_index": 1, "port": 1, "direction": "output", "universe": 0x0456})),
        );
        let s = sends(&cx.take());
        assert_eq!(s.len(), 1);
        let p = &s[0].1;
        assert_eq!(p.len(), 107);
        assert_eq!(hex(&p[..14]), "4172742d4e6574000060000e8401");
        assert_eq!(
            &p[96..107],
            &[0x7f, 0x7f, 0x7f, 0x7f, 0x86, 0x7f, 0x7f, 0x7f, 0x85, 255, 0]
        );
        let mut cx = Cx::new(70);
        m.datagram(&mut cx, SOCKET, from, &node_reply([2, 1, 2, 3]));
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(50)));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
        // Unanswered: a timeout.
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            8,
            "node_command",
            &params(json!({"node": "2.1.2.3", "action": "led_locate", "port": 1})),
        );
        let s = sends(&cx.take());
        assert_eq!(s[0].1[106], 0x04);
        let mut cx = Cx::new(3100);
        m.timer(&mut cx, DEADLINE);
        assert!(cx.take().contains(&Action::Complete {
            id: 8,
            result: Err(CommandError::Timeout)
        }));
    }

    #[test]
    fn input_enable_keeps_the_other_inputs() {
        let mut m = module(json!({}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut reply = node_reply([2, 1, 2, 3]);
        reply[173] = 2;
        reply[179] = 0b0000_1000; // input 2 disabled
        m.datagram(&mut cx, SOCKET, "2.1.2.3:6454".parse().unwrap(), &reply);
        cx.take();
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "set_node_input_enabled",
            &params(json!({"node": "2.1.2.3", "port": 1, "enabled": false})),
        );
        let s = sends(&cx.take());
        assert_eq!(hex(&s[0].1), "4172742d4e6574000070000e0001000401010000");
    }

    #[test]
    fn answers_a_poll_and_ignores_its_own_packets() {
        let mut m = module(json!({"bind_address": "10.0.0.9"}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let own_poll = sends(&cx.take())[0].1.clone();
        let me: SocketAddr = "10.0.0.9:6454".parse().unwrap();
        let mut cx = Cx::new(5);
        m.datagram(&mut cx, SOCKET, me, &own_poll);
        let a = cx.take();
        let s = sends(&a);
        assert_eq!(s.len(), 1, "answers its own poll, as controllers must");
        let reply = s[0].1.clone();
        assert_eq!(reply.len(), 239);
        assert_eq!(&reply[10..14], &[10, 0, 0, 9]);
        assert_eq!(reply[200], 1, "a controller");
        assert_eq!(read_text(&reply[26..44]), "Meros");
        // Its own reply heard back is no node.
        let mut cx = Cx::new(6);
        m.datagram(&mut cx, SOCKET, me, &reply);
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
        assert!(m.nodes.is_empty());
    }

    #[test]
    fn received_dmx_is_kept_only_for_the_universes_named_and_throttled() {
        let mut m = module(
            json!({"receive": "1-2", "receive_interval_ms": 100}),
            None,
            true,
        );
        let from: SocketAddr = "2.9.9.9:6454".parse().unwrap();
        let mut cx = Cx::new(0);
        m.datagram(&mut cx, SOCKET, from, &art_dmx(1, 3, &[1, 2]));
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
        let mut cx = Cx::new(1);
        m.datagram(&mut cx, SOCKET, from, &art_dmx(1, 1, &[1, 2]));
        assert!(cx.take().contains(&Action::State(json!({"inputs": {"1": {
            "channels": [1, 2], "source": "2.9.9.9", "sequence": 1}}}))));
        let mut cx = Cx::new(20);
        m.datagram(&mut cx, SOCKET, from, &art_dmx(2, 1, &[3, 4]));
        let a = cx.take();
        assert!(!a.iter().any(|a| matches!(a, Action::State(_))));
        assert!(a.contains(&Action::SetTimer {
            key: "inputs",
            after: 81
        }));
        let mut cx = Cx::new(101);
        m.timer(&mut cx, "inputs");
        assert!(cx.take().contains(&Action::State(json!({"inputs": {"1": {
            "channels": [3, 4], "source": "2.9.9.9", "sequence": 2}}}))));
        // Commands only: nothing received is kept.
        let mut m = module(json!({"receive": "all"}), None, false);
        let mut cx = Cx::new(0);
        m.datagram(&mut cx, SOCKET, from, &art_dmx(1, 1, &[1]));
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
    }

    #[test]
    fn ip_programming_and_data_requests_return_the_reply() {
        let mut m = module(json!({}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "set_node_ip",
            &params(json!({"node": "2.1.2.3", "ip": "2.1.2.4", "subnet_mask": "255.0.0.0", "gateway": "2.0.0.1"})),
        );
        let s = sends(&cx.take());
        assert_eq!(s[0].0, "2.1.2.3:6454".parse::<SocketAddr>().unwrap());
        assert_eq!(
            hex(&s[0].1),
            "4172742d4e65740000f8000e0000960002010204ff00000000000200000100000000"
        );
        let mut reply = header(op::IP_PROG_REPLY);
        reply.extend_from_slice(&[
            0, 0, 0, 0, 2, 1, 2, 4, 255, 0, 0, 0, 0, 0, 0x40, 0, 2, 0, 0, 1, 0, 0,
        ]);
        let mut cx = Cx::new(30);
        m.datagram(&mut cx, SOCKET, "2.1.2.3:6454".parse().unwrap(), &reply);
        assert!(cx.take().contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"ip": "2.1.2.4", "subnet_mask": "255.0.0.0", "gateway": "2.0.0.1", "dhcp": true})
            })
        }));
        let mut cx = Cx::new(40);
        m.command(
            &mut cx,
            2,
            "get_node_data",
            &params(json!({"node": "2.1.2.3", "request": "product_url"})),
        );
        let s = sends(&cx.take());
        assert_eq!(hex(&s[0].1[..18]), "4172742d4e6574000027000e000000ff0001");
        let mut reply = header(op::DATA_REPLY);
        reply.extend_from_slice(&[0, 0, 0, 0xff, 0, 1, 0, 6]);
        reply.extend_from_slice(b"a.com\0");
        let mut cx = Cx::new(41);
        m.datagram(&mut cx, SOCKET, "2.1.2.3:6454".parse().unwrap(), &reply);
        assert!(cx.take().contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Value {
                value: json!("a.com")
            })
        }));
    }

    #[test]
    fn tod_data_builds_the_uid_list_across_blocks() {
        let mut m = module(json!({}), None, true);
        let tod = |block: u8, uids: &[[u8; 6]]| {
            let mut p = header(op::TOD_DATA);
            p.extend_from_slice(&[
                1,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
                0,
                0,
                0x05,
                0,
                3,
                block,
                uids.len() as u8,
            ]);
            for u in uids {
                p.extend_from_slice(u);
            }
            p
        };
        let from: SocketAddr = "2.1.2.3:6454".parse().unwrap();
        let mut cx = Cx::new(0);
        m.datagram(
            &mut cx,
            SOCKET,
            from,
            &tod(0, &[[0x41, 0x4c, 0, 0, 0, 1], [0x41, 0x4c, 0, 0, 0, 2]]),
        );
        m.datagram(&mut cx, SOCKET, from, &tod(1, &[[0x41, 0x4c, 0, 0, 0, 3]]));
        let last = cx
            .take()
            .into_iter()
            .rev()
            .find_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            last,
            json!({"rdm": {"5": {"available": true, "total": 3, "gateway": "2.1.2.3",
                "uids": ["414c00000001", "414c00000002", "414c00000003"]}}})
        );
    }

    #[test]
    fn triggers_commands_and_timecode_are_encoded_as_specified() {
        let mut m = module(json!({"poll_addresses": "2.255.255.255"}), None, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "send_trigger",
            &params(json!({"key": 1, "sub_key": 7, "oem": 65535, "payload": ""})),
        );
        let s = sends(&cx.take());
        assert_eq!(s[0].1.len(), 530);
        assert_eq!(hex(&s[0].1[..18]), "4172742d4e6574000099000e0000ffff0107");
        let mut cx = Cx::new(2);
        m.command(
            &mut cx,
            2,
            "send_command",
            &params(json!({"text": "SwoutText=Playback&", "esta": 65535})),
        );
        let s = sends(&cx.take());
        assert_eq!(hex(&s[0].1[..16]), "4172742d4e6574000024000effff0014");
        let mut cx = Cx::new(3);
        m.command(
            &mut cx,
            3,
            "send_timecode",
            &params(json!({"hours": 1, "minutes": 2, "seconds": 3, "frames": 4, "type": "ebu", "stream": 0, "target": "2.0.0.7"})),
        );
        let s = sends(&cx.take());
        assert_eq!(s[0].0, "2.0.0.7:6454".parse::<SocketAddr>().unwrap());
        assert_eq!(hex(&s[0].1), "4172742d4e6574000097000e00000403020101");
        // A timecode heard from another device.
        let mut cx = Cx::new(4);
        m.datagram(
            &mut cx,
            SOCKET,
            "2.0.0.8:6454".parse().unwrap(),
            &art_time_code(0, 4, 3, 2, 1, 3),
        );
        assert!(cx.take().contains(&Action::State(json!({"timecode": {
            "hours": 1, "minutes": 2, "seconds": 3, "frames": 4, "type": "smpte", "stream": 0, "source": "2.0.0.8"}}))));
    }
}
