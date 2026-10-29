//! Panasonic projectors over their control commands, on TCP 1024
//! (NTCONTROL) or a serial port.
//!
//! Protocol from the "Control commands via LAN" and RS-232C chapters of
//! Panasonic's projector Operating Instructions (here the PT-RZ120 Functional
//! Manual, DPQP1239ZF) and the per-series Control Commands lists, which give
//! every command, query and reply.
//!
//! - Over the network each command is a session of its own: the client
//!   connects, the projector says `NTCONTROL 1 <8 random characters>` CR in
//!   protect mode (an administrator password set for web control) or
//!   `NTCONTROL 0` CR otherwise; the client sends, in protect mode, the
//!   32-character MD5 hash of `<user>:<password>:<random>`, then `00`, the
//!   command and CR, or without protection `00`, the command and CR; the
//!   projector answers `00`, the reply and CR, and closes the connection.
//!   Errors are `ERR1` (undefined command), `ERR2` (out of range), `ERR3`
//!   (busy or not accepted now), `ERR4` (timeout), `ERR5` (wrong length) and
//!   `ERRA` (password mismatch). `ERRA` is terminal: nothing is sent again
//!   until the device is opened again with corrected credentials.
//! - On the serial port a command is STX, `AD`, the projector ID (`ZZ` for
//!   all, `01`-`64`, or a group `0A`-`0Z`), `;`, the command, ETX; the
//!   answer is STX, the reply, ETX, or `ER401` (cannot be executed now) or
//!   `ER402` (invalid parameter). Commands are sent 0.5 s after the previous
//!   answer, and 60 s after the light source starts lighting, as the manual
//!   asks.
//! - The projector pushes nothing, so state is polled: the power status
//!   every 5 s, and while the projector is on, every query the model's
//!   command list has, one sweep a minute at most.
//!
//! Opened for commands only (`monitor` false), only the power query runs
//! every 5 s, as the liveness check (the projector sends nothing unasked,
//! and over the network each command is its own connection). Replies to
//! commands still update the state, and every answered request reports its
//! round trip as the device's latency.

use std::collections::VecDeque;
use std::net::SocketAddr;

use md5::{Digest, Md5};
use serde_json::{json, Value};

use super::panasonic_projector_commands::COMMANDS;
use crate::catalog::{DeviceSpec, Params};
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, SerialPort, TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 1024;
const SOCKET: Key = "ntcontrol";
const REPLY: Key = "reply";
const POLL: Key = "poll";
const RETRY: Key = "retry";
const PACE: Key = "pace";

/// The manual asks for a time-out of 10 s or more.
const REPLY_TIMEOUT: Millis = 10_000;
const POLL_EVERY: Millis = 5_000;
const SWEEP_EVERY: u32 = 12;
/// Serial: 0.5 s after an answer before the next command.
const SERIAL_GAP: Millis = 500;
/// Serial: 60 s after the light source starts lighting before the next
/// command (RS-232C "Attention").
const LIGHTING_GAP: Millis = 60_000;
const MISSES: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const MAX_REPLY: usize = 4096;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;

/// How a generated command is sent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    /// The prefix (`IIS:` or `VXX:LNSI2=`) and the `value` parameter.
    Set,
    /// The code alone.
    Action,
    /// A query, whose reply is kept in state.
    Get,
}

fn lookup(name: &str) -> Option<(&'static str, Kind)> {
    COMMANDS
        .binary_search_by(|(n, _, _)| (*n).cmp(name))
        .ok()
        .map(|i| (COMMANDS[i].1, COMMANDS[i].2))
}

/// Each series' inputs, by the values set_input_select takes, as its control
/// command list's INPUT SELECT rows give them (the rows the list marks for
/// the series). Sorted by model id.
#[rustfmt::skip]
const INPUTS: &[(&str, &[&str])] = &[
    ("pt-cmz50", &["RG1", "HD1", "HD2", "NWP", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-dz13k-ds12k-dw11k-dz10k", &["RG1", "RG2", "VID", "DVI", "HD1", "SD1", "SD2"]),
    ("pt-dz21k2-ds20k2-dw17k2-dz16k2-dz21k-dz20k-dz16k-dw17k", &["RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SD1", "SD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-dz780-dw750-dx820", &["RG1", "RG2", "VID", "DVI", "HD1", "SD1", "DL1", "DL1:HD1", "DL1:HD2", "DL1:PC1", "DL1:PC2", "DL1:SVD", "DL1:VID"]),
    ("pt-dz870-dw830-dx100", &["RG1", "RG2", "VID", "DL1", "DVI", "HD1", "SD1", "DL1:HD1", "DL1:HD2", "DL1:PC1", "DL1:PC2", "DL1:SVD", "DL1:VID"]),
    ("pt-ex12k", &["PC1", "PC2", "RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SCT", "AU1,RG1", "AU2,RG1", "AU1,VID", "AU2,VID", "AU1,SVD", "AU2,SVD", "AU1,RG2", "AU2,RG2", "AU1,SCT", "AU2,SCT", "AU1,DVI", "AU2,DVI", "AU1,SD1", "AU2,SD1", "AU1,SD2", "AU2,SD2"]),
    ("pt-ex16k", &["PC1", "PC2", "RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SCT", "AU1,RG1", "AU2,RG1", "AU1,VID", "AU2,VID", "AU1,SVD", "AU2,SVD", "AU1,RG2", "AU2,RG2", "AU1,SCT", "AU2,SCT", "AU1,DVI", "AU2,DVI", "AU1,SD1", "AU2,SD1", "AU1,SD2", "AU2,SD2"]),
    ("pt-ez590", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-ez770z-ez770zl", &["RG1", "RG2", "VID", "SVD", "DVI", "HD1", "DP1", "DL1", "DL1:HD1", "DL1:HD2", "DL1:PC1", "DL1:PC2", "DL1:SVD", "DL1:VID"]),
    ("pt-frq50", &["RG1", "HD1", "HD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-frq60", &["RG1", "HD1", "HD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-frz55", &["RG1", "RG2", "VID", "HD1", "HD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2"]),
    ("pt-frz60-frz50", &["RG1", "RG2", "VID", "HD1", "HD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2"]),
    ("pt-htq20", &["HD1", "HD2", "SD1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,MP1", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-jx200", &["HD1", "PA1", "MC1", "MV1", "SI1"]),
    ("pt-lb425", &["RG1", "RG2", "VID", "SVD", "CP1", "HD1", "NWP", "MV1", "UD1"]),
    ("pt-lb426", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "MV1"]),
    ("pt-lmz460", &["RG1", "RG2", "HD1", "HD2", "NWP", "MV1"]),
    ("pt-lrz35", &["RG1", "RG2", "VID", "HD1", "HD2"]),
    ("pt-mz16k", &["RG1", "DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-mz20k", &["HD1", "HD2", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-mz770-mz670", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "PA1", "MC1", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-mz880", &["RG1", "RG2", "HD1", "HD3", "DL1", "DL1:PC1", "DL1:HD1", "DL1:HD2"]),
    ("pt-mz882", &["RG1", "RG2", "HD1", "HD2", "HD3", "DL1", "DL1:PC1", "DL1:HD1", "DL1:HD2"]),
    ("pt-rcq10", &["DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD", "AU1,SD1", "AU1,SD2", "AU1,SD3", "AU1,SD4", "AU1,HD1", "AU1,HD2", "AU1,DV1", "AU1,DV2", "AU1,DP1", "AU1,DP2", "AU1,OP1", "AU1,OP2"]),
    ("pt-req15", &["HD1", "HD2", "DP1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rez15", &["HD1", "HD2", "DP1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rq25k", &["HD1", "HD2", "DP1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rq32k", &["RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SD1", "SD2", "SD3", "SD4", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD", "AU1,SD1", "AU1,SD2", "AU1,SD3", "AU1,SD4", "AU2,SD1", "AU2,SD2", "AU2,SD3", "AU2,SD4", "AU1,HD1", "AU1,HD2", "AU2,HD3", "AU2,HD4", "AU1,DV1", "AU1,DV2", "AU2,DV3", "AU2,DV4", "AU1,DP1", "AU1,DP2", "AU2,DP3", "AU2,DP4", "AU1,OP1", "AU1,OP2", "AU2,OP1", "AU2,OP2"]),
    ("pt-rq35k", &["DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD", "AU1,SD1", "AU1,SD2", "AU1,SD3", "AU1,SD4", "AU2,SD1", "AU2,SD2", "AU2,SD3", "AU2,SD4", "AU1,HD1", "AU1,HD2", "AU2,HD3", "AU2,HD4", "AU1,DV1", "AU1,DV2", "AU2,DV3", "AU2,DV4", "AU1,DP1", "AU1,DP2", "AU2,DP3", "AU2,DP4", "AU1,OP1", "AU1,OP2", "AU2,OP1", "AU2,OP2"]),
    ("pt-rq35k2", &["DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD", "AU1,SD1", "AU1,SD2", "AU1,SD3", "AU1,SD4", "AU2,SD1", "AU2,SD2", "AU2,SD3", "AU2,SD4", "AU1,HD1", "AU1,HD2", "AU2,HD3", "AU2,HD4", "AU1,DV1", "AU1,DV2", "AU2,DV3", "AU2,DV4", "AU1,DP1", "AU1,DP2", "AU2,DP3", "AU2,DP4", "AU1,OP1", "AU1,OP2", "AU2,OP1", "AU2,OP2"]),
    ("pt-rq45k", &["HD1", "HD2", "SD1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rq50k", &["DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD", "AU1,SD1", "AU1,SD2", "AU1,SD3", "AU1,SD4", "AU2,SD1", "AU2,SD2", "AU2,SD3", "AU2,SD4", "AU1,HD1", "AU1,HD2", "AU2,HD3", "AU2,HD4", "AU1,DV1", "AU1,DV2", "AU2,DV3", "AU2,DV4", "AU1,DP1", "AU1,DP2", "AU2,DP3", "AU2,DP4", "AU1,OP1", "AU1,OP2", "AU2,OP1", "AU2,OP2"]),
    ("pt-rq7", &["HD1", "HD2", "DL1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rz120", &["RG1", "RG2", "DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-rz14k", &["HD1", "HD2", "DP1", "DM1,SD1", "DM1,DL1", "DM1,WP1", "DM1,TP1", "DM1,OP1", "DM1,OP2", "DM1,DL1:PC1", "DM1,DL1:PC2", "DM1,DL1:VID", "DM1,DL1:HD1", "DM1,DL1:HD2", "DM1,DL1:SVD"]),
    ("pt-rz16k", &["RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SD1", "SD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-rz470-rw430-rz475-frz370c-frw330c", &["PC1", "PC2", "RG1", "RG2", "VID", "DVI", "HD1", "DL1", "DL1:HD1", "DL1:HD2", "DL1:PC1", "DL1:PC2", "DL1:SVD", "DL1:VID"]),
    ("pt-rz570-rz575", &["RG1", "RG2", "VID", "DVI", "HD1", "HD2", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2"]),
    ("pt-rz970-rw930-rx110", &["RG1", "RG2", "VID", "SVD", "DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-rz990", &["RG1", "RG2", "DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-tmz400", &["RG1", "RG2", "HD1", "HD2", "NWP", "MV1"]),
    ("pt-vmq85", &["HD1", "HD2", "NWP", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-vmz60-vmz50-vmz40-vmw60-vmw50", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-vmz71", &["RG1", "RG2", "HD1", "HD2", "NWP", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-vmz82", &["RG1", "HD1", "HD2", "NWP", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-vw360", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "MV1", "UD1"]),
    ("pt-vz575n-vw535n-vx605n-vz570-vw530-vx600-vz470", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "PA1", "MC1", "MV1", "DL1", "DL1:RG1", "DL1:RG2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
    ("pt-vz585n-vz580-vw545n-vw540-vx615n-vx610n", &["RG1", "RG2", "VID", "HD1", "HD2", "NWP", "PA1", "MC1", "MV1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID", "DL1:HD1", "DL1:HD2", "DL1:SVD"]),
];

/// The model's inputs from INPUTS; none for an unknown model.
fn model_inputs(model: &str) -> &'static [&'static str] {
    INPUTS
        .binary_search_by(|(m, _)| (*m).cmp(model))
        .map(|i| INPUTS[i].1)
        .unwrap_or(&[])
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    line: String,
    /// The state key of a query's reply (the command name without `get_`).
    key: Option<&'static str>,
    follow: Vec<Job>,
}

impl Job {
    fn query(line: &str, key: Option<&'static str>) -> Job {
        Job {
            command: None,
            line: line.to_string(),
            key,
            follow: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// TCP: connecting, then waiting for `NTCONTROL`.
    Connecting,
    Greeting,
    /// The command is sent; its answer is awaited.
    Sent,
    /// Serial: the port is opening.
    Opening,
    Closed,
}

pub(crate) struct PanasonicProjector {
    tcp: Option<SocketAddr>,
    serial: Option<SerialPort>,
    id: String,
    user: String,
    password: String,
    monitor: bool,
    /// The model's queries: (query, state key).
    reads: Vec<(&'static str, &'static str)>,
    /// The model's inputs, as set_input_select takes them.
    inputs: &'static [&'static str],
    phase: Phase,
    port_open: bool,
    reachable: bool,
    powered: bool,
    /// The power has been read since the link opened.
    power_known: bool,
    refused: Option<String>,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
    paced: bool,
    cycle: u32,
}

impl PanasonicProjector {
    pub(crate) fn new(spec: &DeviceSpec, ctx: OpenContext) -> Result<PanasonicProjector, String> {
        let model = spec
            .models
            .iter()
            .find(|m| m.id == ctx.model)
            .ok_or_else(|| format!("no Panasonic projector model '{}'", ctx.model))?;
        let reads = model
            .supports
            .iter()
            .filter_map(|c| match lookup(c) {
                Some((q, Kind::Get)) => COMMANDS
                    .iter()
                    .find(|(n, _, _)| n == c)
                    .map(|(n, _, _)| (q, &n[4..])),
                _ => None,
            })
            .collect();
        let s = &ctx.settings;
        let text = |k: &str, d: &str| s.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let mut m = PanasonicProjector::with(link, reads, ctx.monitor);
        m.inputs = model_inputs(&model.id);
        m.id = text("projector_id", "ZZ");
        m.user = text("username", "dispadmin");
        m.password = text("password", "");
        Ok(m)
    }

    fn with(link: Link, reads: Vec<(&'static str, &'static str)>, monitor: bool) -> Self {
        let (tcp, serial) = match link {
            Link::Tcp(a) => (Some(a), None),
            Link::Serial(p) => (None, Some(p)),
        };
        PanasonicProjector {
            tcp,
            serial,
            id: "ZZ".into(),
            user: "dispadmin".into(),
            password: String::new(),
            monitor,
            reads,
            inputs: &[],
            phase: Phase::Idle,
            port_open: false,
            reachable: false,
            powered: false,
            power_known: false,
            refused: None,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            paced: false,
            cycle: 0,
        }
    }

    fn queue_cycle(&mut self, all: bool) {
        if self.polls.iter().any(|j| j.line == "QPW") {
            return;
        }
        self.polls.push_back(Job::query("QPW", None));
        if !self.monitor && !all {
            return;
        }
        // One sweep at a time, at most every minute, while the projector is
        // on (in standby most queries are refused).
        let sweeping = self.polls.len() > 1;
        if (self.powered || all) && !sweeping && (self.cycle.is_multiple_of(SWEEP_EVERY) || all) {
            for (q, key) in self.reads.clone() {
                if q != "QPW" {
                    self.polls.push_back(Job::query(q, Some(key)));
                }
            }
        }
        self.cycle = self.cycle.wrapping_add(1);
    }

    /// Send the next job: over TCP open a connection for it, on serial
    /// send it on the open port.
    fn pump(&mut self, cx: &mut Cx) {
        if self.refused.is_some() || self.in_flight.is_some() || self.paced {
            return;
        }
        if self.tcp.is_some() && self.phase != Phase::Idle {
            return;
        }
        if self.serial.is_some() && (!self.port_open || self.phase != Phase::Idle) {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        self.buf.clear();
        cx.set_timer(REPLY, REPLY_TIMEOUT);
        if let Some(addr) = self.tcp {
            self.phase = Phase::Connecting;
            cx.tcp_open(SOCKET, addr);
        } else {
            let mut out = vec![STX];
            out.extend_from_slice(format!("AD{};{}", self.id, job.line).as_bytes());
            out.push(ETX);
            cx.tcp_send(SOCKET, out);
            self.phase = Phase::Sent;
        }
        self.in_flight = Some((job, cx.now()));
    }

    fn fail_all(&mut self, cx: &mut Cx, err: impl Fn() -> CommandError) {
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.commands.drain(..))
        {
            if let Some(id) = job.command {
                cx.complete(id, Err(err()));
            }
        }
        self.polls.clear();
    }

    /// The projector stopped answering or the port failed.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let r = reason.clone();
        self.fail_all(cx, || CommandError::Transport { message: r.clone() });
        cx.cancel_timer(REPLY);
        cx.tcp_close(SOCKET);
        self.buf.clear();
        self.misses = 0;
        self.reachable = false;
        self.power_known = false;
        self.port_open = false;
        self.phase = Phase::Closed;
        cx.log(Level::Debug, format!("Panasonic projector: {reason}"));
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        let r = reason.clone();
        self.fail_all(cx, || CommandError::Auth { message: r.clone() });
        cx.cancel_timer(REPLY);
        cx.cancel_timer(POLL);
        cx.tcp_close(SOCKET);
        self.phase = Phase::Closed;
        cx.log(
            Level::Warning,
            format!("{reason}; nothing more is sent until the device is opened again with corrected credentials"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.clone(),
        });
        self.refused = Some(reason);
    }

    fn greeting(&mut self, cx: &mut Cx, line: &str) {
        let Some((job, _)) = &self.in_flight else {
            return;
        };
        let mut out = String::new();
        if let Some(random) = line.strip_prefix("NTCONTROL 1 ") {
            if self.password.is_empty() {
                self.refuse(
                    cx,
                    "the projector is in protect mode and no password is set".into(),
                );
                return;
            }
            out.push_str(&ntcontrol_hash(&self.user, &self.password, random.trim()));
        } else if line.trim() != "NTCONTROL 0" {
            self.lost(cx, format!("unexpected greeting '{line}'"));
            return;
        }
        out.push_str("00");
        out.push_str(&job.line);
        out.push('\r');
        cx.tcp_send(SOCKET, out.into_bytes());
        self.phase = Phase::Sent;
    }

    fn answered(&mut self, cx: &mut Cx, reply: &str) {
        let Some((job, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        cx.alive();
        self.misses = 0;
        self.retry_after = RETRY_MIN;
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        // Over the network the reply carries "00" before the data.
        let reply = match self.tcp {
            Some(_) => reply.strip_prefix("00").unwrap_or(reply),
            None => reply,
        };
        // The light source starts lighting: power on accepted while not
        // known to be on, or the power read on after it was read standby.
        let was_on = self.power_known && self.powered;
        let lighting = error_meaning(reply).is_none()
            && !was_on
            && (job.line == "PON" || (job.line == "QPW" && reply == "001" && self.power_known));
        if job.line == "PON" && error_meaning(reply).is_none() {
            // On from now: the read that follows does not light it again.
            self.powered = true;
            self.power_known = true;
        }
        let result = match error_meaning(reply) {
            Some(meaning) => Err(CommandError::DeviceError {
                code: Some(reply.to_string()),
                message: format!("{}: {meaning}", job.line),
            }),
            None => {
                if job.line == "QPW" || job.key.is_some() {
                    let mut patch = json!({});
                    if job.line == "QPW" {
                        self.powered = reply == "001";
                        self.power_known = true;
                        patch["power"] = json!(match reply {
                            "000" => "standby",
                            "001" => "on",
                            other => other,
                        });
                        patch["power_code"] = json!(reply);
                    }
                    if let Some(key) = job.key {
                        patch["settings"] = json!({ key: reply });
                    }
                    cx.state(patch);
                    Ok(Outcome::Value {
                        value: json!(reply),
                    })
                } else {
                    Ok(Outcome::Ack)
                }
            }
        };
        if result.is_ok() && self.monitor {
            for follow in job.follow.iter().rev() {
                self.polls.push_front(follow.clone());
            }
        }
        if let Some(id) = job.command {
            cx.complete(id, result);
        }
        if self.tcp.is_some() {
            cx.tcp_close(SOCKET);
            self.phase = Phase::Idle;
            self.pump(cx);
        } else {
            self.phase = Phase::Idle;
            self.paced = true;
            cx.set_timer(PACE, if lighting { LIGHTING_GAP } else { SERIAL_GAP });
        }
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() > MAX_REPLY {
            self.lost(cx, "an answer longer than the protocol allows".into());
            return;
        }
        if self.tcp.is_some() {
            while let Some(end) = self.buf.iter().position(|&b| b == b'\r') {
                let raw: Vec<u8> = self.buf.drain(..=end).collect();
                let line = String::from_utf8_lossy(&raw[..raw.len() - 1])
                    .trim()
                    .to_string();
                match self.phase {
                    Phase::Greeting => self.greeting(cx, &line),
                    Phase::Sent => {
                        if line == "ERRA" {
                            self.refuse(
                                cx,
                                "the projector refused the user name or password (ERRA)".into(),
                            );
                        } else {
                            self.answered(cx, &line);
                        }
                    }
                    _ => {}
                }
                if self.phase == Phase::Closed || self.phase == Phase::Idle {
                    self.buf.clear();
                    return;
                }
            }
        } else {
            while let (Some(s), Some(e)) = (
                self.buf.iter().position(|&b| b == STX),
                self.buf.iter().position(|&b| b == ETX),
            ) {
                if e < s {
                    self.buf.drain(..=e);
                    continue;
                }
                let frame: Vec<u8> = self.buf.drain(..=e).collect();
                let text = String::from_utf8_lossy(&frame[s + 1..frame.len() - 1]).to_string();
                if self.phase == Phase::Sent {
                    self.answered(cx, &text);
                }
            }
        }
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        if name == "set_power" {
            let on = p
                .get("on")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("'on' is required"))?;
            let mut job = Job::query(if on { "PON" } else { "POF" }, None);
            job.follow.push(Job::query("QPW", None));
            return Ok(job);
        }
        if name == "get_power" {
            return Ok(Job::query("QPW", None));
        }
        let (wire, kind) = lookup(name).ok_or_else(|| CommandError::UnknownCommand {
            command: name.into(),
        })?;
        Ok(match kind {
            Kind::Action => Job::query(wire, None),
            Kind::Get => {
                let key = COMMANDS
                    .iter()
                    .find(|(n, _, _)| *n == name)
                    .map(|(n, _, _)| &n[4..]);
                Job::query(wire, key)
            }
            Kind::Set => {
                let v = p
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("'value' is required"))?;
                if v.is_empty() || !v.bytes().all(|b| (0x21..0x7F).contains(&b)) {
                    return Err(invalid("the value is printable ASCII without spaces"));
                }
                let mut job = Job::query(&format!("{wire}{v}"), None);
                if let Some(q) = self.query_for(name) {
                    job.follow.push(q);
                }
                job
            }
        })
    }

    /// The query a set is read back with: the `get_` of the same name.
    fn query_for(&self, set: &str) -> Option<Job> {
        let get = format!("get_{}", set.strip_prefix("set_")?);
        let (q, kind) = lookup(&get)?;
        (kind == Kind::Get).then(|| {
            let key = COMMANDS
                .iter()
                .find(|(n, _, _)| *n == get)
                .map(|(n, _, _)| &n[4..])
                .expect("found above");
            Job::query(q, Some(key))
        })
    }
}

/// `<user>:<password>:<random>` hashed with MD5, as 32 lowercase hex digits.
fn ntcontrol_hash(user: &str, password: &str, random: &str) -> String {
    Md5::digest(format!("{user}:{password}:{random}").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn error_meaning(reply: &str) -> Option<&'static str> {
    Some(match reply {
        "ERR1" => "undefined control command",
        "ERR2" => "out of parameter range",
        "ERR3" => "busy, or not accepted now",
        "ERR4" => "timeout, or not accepted now",
        "ERR5" => "wrong data length",
        "ER401" => "the command cannot be executed now",
        "ER402" => "invalid parameter",
        _ => return None,
    })
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

impl Module for PanasonicProjector {
    fn start(&mut self, cx: &mut Cx) {
        if self.monitor && !self.inputs.is_empty() {
            // The series' inputs, from its control command list: no query
            // lists the inputs a projector has.
            cx.state(json!({ "inputs": { "available": self.inputs } }));
        }
        cx.connection(Connection::Connecting);
        if let Some(port) = self.serial.clone() {
            self.phase = Phase::Opening;
            cx.serial_open(SOCKET, port);
        } else {
            self.queue_cycle(false);
            self.pump(cx);
        }
        cx.set_timer(POLL, POLL_EVERY);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(reason) = &self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: reason.clone(),
                }),
            );
            return;
        }
        if name == "refresh" {
            self.polls.clear();
            self.queue_cycle(true);
            cx.complete(id, Ok(Outcome::Ack));
            self.pump(cx);
            return;
        }
        match self.command_job(name, params) {
            Ok(mut job) => {
                job.command = Some(id);
                self.commands.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            TcpInput::Connected => {
                if self.serial.is_some() {
                    if self.phase == Phase::Opening {
                        self.port_open = true;
                        self.phase = Phase::Idle;
                        self.queue_cycle(false);
                        self.pump(cx);
                    }
                } else if self.phase == Phase::Connecting {
                    self.phase = Phase::Greeting;
                }
            }
            TcpInput::Data(bytes) => {
                if matches!(self.phase, Phase::Greeting | Phase::Sent) {
                    self.data(cx, &bytes);
                }
            }
            TcpInput::Closed { reason } => {
                // Over TCP the projector closes each connection after
                // answering; only a close before the answer is a failure.
                let failed = self.serial.is_some()
                    || matches!(
                        self.phase,
                        Phase::Connecting | Phase::Greeting | Phase::Sent
                    );
                if failed && self.phase != Phase::Closed {
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused.is_some() {
            return;
        }
        match key {
            RETRY => {
                if self.serial.is_some() {
                    self.phase = Phase::Opening;
                    cx.connection(Connection::Connecting);
                    cx.serial_open(SOCKET, self.serial.clone().expect("serial"));
                } else {
                    self.phase = Phase::Idle;
                    self.queue_cycle(false);
                    self.pump(cx);
                }
            }
            PACE => {
                self.paced = false;
                self.pump(cx);
            }
            POLL => {
                cx.set_timer(POLL, POLL_EVERY);
                if self.phase != Phase::Closed {
                    self.queue_cycle(false);
                    self.pump(cx);
                }
            }
            REPLY => {
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.tcp.is_some() || self.misses >= MISSES {
                        if self.tcp.is_some() && self.misses < MISSES && self.reachable {
                            // One lost exchange: try the next on a new connection.
                            cx.tcp_close(SOCKET);
                            self.phase = Phase::Idle;
                            self.pump(cx);
                            return;
                        }
                        self.lost(cx, "the projector stopped answering".into());
                        return;
                    }
                    self.phase = Phase::Idle;
                    self.pump(cx);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn tcp_module(monitor: bool, password: &str) -> PanasonicProjector {
        let mut m = PanasonicProjector::with(
            Link::Tcp(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 60)),
                DEFAULT_PORT,
            )),
            vec![("QIN", "input_select")],
            monitor,
        );
        m.password = password.into();
        m
    }

    fn serial_module() -> PanasonicProjector {
        PanasonicProjector::with(
            Link::Serial(
                SerialPort::from_settings(json!({"serial_port": "COM4"}).as_object().unwrap())
                    .unwrap(),
            ),
            vec![],
            false,
        )
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

    fn opens(actions: &[Action]) -> usize {
        actions
            .iter()
            .filter(|a| matches!(a, Action::TcpOpen { .. }))
            .count()
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

    fn input(m: &mut PanasonicProjector, i: TcpInput) -> Vec<Action> {
        let mut cx = Cx::new(25);
        m.tcp(&mut cx, SOCKET, i);
        cx.take()
    }

    fn data(m: &mut PanasonicProjector, d: &[u8]) -> Vec<Action> {
        input(m, TcpInput::Data(d.to_vec()))
    }

    #[test]
    fn the_hash_is_md5_of_user_password_and_random() {
        assert_eq!(
            ntcontrol_hash("dispadmin", "@Panasonic", "23181e1e"),
            "a02b37e0ff1181c13808c49c7fd9af80"
        );
    }

    #[test]
    fn a_protected_session_sends_the_hash_then_the_command() {
        let mut m = tcp_module(true, "@Panasonic");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert_eq!(opens(&a), 1);
        input(&mut m, TcpInput::Connected);
        let a = data(&mut m, b"NTCONTROL 1 23181e1e\r");
        assert_eq!(
            sent(&a),
            [b"a02b37e0ff1181c13808c49c7fd9af8000QPW\r".to_vec()]
        );
        let a = data(&mut m, b"00001\r");
        assert_eq!(state(&a)["power"], "on");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(25)));
        // Each command is a connection of its own: the next opens again.
        assert!(a.iter().any(|x| matches!(x, Action::TcpClose { .. })));
    }

    #[test]
    fn an_unprotected_session_sends_00_and_the_command() {
        let mut m = tcp_module(false, "");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        input(&mut m, TcpInput::Connected);
        let a = data(&mut m, b"NTCONTROL 0\r");
        assert_eq!(sent(&a), [b"00QPW\r".to_vec()]);
        data(&mut m, b"00000\r");
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            4,
            "set_input_select",
            json!({"value": "HD1"}).as_object().unwrap(),
        );
        assert_eq!(opens(&cx.take()), 1);
        input(&mut m, TcpInput::Connected);
        let a = data(&mut m, b"NTCONTROL 0\r");
        assert_eq!(sent(&a), [b"00IIS:HD1\r".to_vec()]);
        let a = data(&mut m, b"00IIS:HD1\r");
        assert_eq!(completed(&a, 4), Some(Ok(Outcome::Ack)));
        // Commands only: no read-back.
        assert_eq!(opens(&a), 0);
    }

    #[test]
    fn erra_refuses_and_err_codes_fail_the_command() {
        let mut m = tcp_module(false, "wrong");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        input(&mut m, TcpInput::Connected);
        data(&mut m, b"NTCONTROL 1 abcdef12\r");
        let a = data(&mut m, b"ERRA\r");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        let mut m = tcp_module(false, "");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.command(
            &mut cx,
            2,
            "set_input_select",
            json!({"value": "XX9"}).as_object().unwrap(),
        );
        input(&mut m, TcpInput::Connected);
        data(&mut m, b"NTCONTROL 0\r");
        data(&mut m, b"00000\r");
        input(&mut m, TcpInput::Connected);
        data(&mut m, b"NTCONTROL 0\r");
        let a = data(&mut m, b"ERR2\r");
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn serial_frames_carry_stx_the_id_and_etx_and_are_paced() {
        let mut m = serial_module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = input(&mut m, TcpInput::Connected);
        assert_eq!(sent(&a), [b"\x02ADZZ;QPW\x03".to_vec()]);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "set_power",
            json!({"on": true}).as_object().unwrap(),
        );
        assert!(sent(&cx.take()).is_empty());
        let a = data(&mut m, b"\x02000\x03");
        assert_eq!(state(&a)["power"], "standby");
        // The next command waits 0.5 s.
        assert!(sent(&a).is_empty());
        let mut cx = Cx::new(500);
        m.timer(&mut cx, PACE);
        assert_eq!(sent(&cx.take()), [b"\x02ADZZ;PON\x03".to_vec()]);
        let a = data(&mut m, b"\x02ER401\x03");
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn serial_commands_wait_60_s_once_the_light_source_lights() {
        let mut m = serial_module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        input(&mut m, TcpInput::Connected);
        data(&mut m, b"000");
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_power",
            json!({"on": true}).as_object().unwrap(),
        );
        let mut cx = Cx::new(500);
        m.timer(&mut cx, PACE);
        assert_eq!(sent(&cx.take()), [b"ADZZ;PON".to_vec()]);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            2,
            "set_input_select",
            json!({"value": "HD1"}).as_object().unwrap(),
        );
        let a = data(&mut m, b"PON");
        assert!(a.contains(&Action::SetTimer {
            key: PACE,
            after: LIGHTING_GAP
        }));
        assert!(sent(&a).is_empty());
        let mut cx = Cx::new(60_000);
        m.timer(&mut cx, PACE);
        assert_eq!(sent(&cx.take()), [b"ADZZ;IIS:HD1".to_vec()]);
        data(&mut m, b"IIS:HD1");
        // Read on afterwards: the light source is not lit again.
        m.polls.push_back(Job::query("QPW", None));
        let mut cx = Cx::new(0);
        m.timer(&mut cx, PACE);
        let a = data(&mut m, b"001");
        assert!(a.contains(&Action::SetTimer {
            key: PACE,
            after: SERIAL_GAP
        }));
        // Already on: power on lights nothing, and the gap is 0.5 s.
        m.commands.push_back(Job::query("PON", None));
        let mut cx = Cx::new(0);
        m.timer(&mut cx, PACE);
        let a = data(&mut m, b"PON");
        assert!(a.contains(&Action::SetTimer {
            key: PACE,
            after: SERIAL_GAP
        }));
    }

    #[test]
    fn serial_commands_wait_60_s_after_the_power_is_read_turning_on() {
        let mut m = serial_module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        // The first read says nothing about lighting.
        input(&mut m, TcpInput::Connected);
        let a = data(&mut m, b"001");
        assert!(a.contains(&Action::SetTimer {
            key: PACE,
            after: SERIAL_GAP
        }));
        m.powered = false;
        m.polls.push_back(Job::query("QPW", None));
        let mut cx = Cx::new(0);
        m.timer(&mut cx, PACE);
        let a = data(&mut m, b"001");
        assert!(a.contains(&Action::SetTimer {
            key: PACE,
            after: LIGHTING_GAP
        }));
    }

    #[test]
    fn the_input_table_is_sorted() {
        for pair in INPUTS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn the_series_inputs_reach_state() {
        let mut m = tcp_module(true, "");
        m.inputs = model_inputs("pt-rz120");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert_eq!(
            state(&cx.take())["inputs"]["available"],
            json!([
                "RG1", "RG2", "DVI", "HD1", "SD1", "DL1", "DL1:PC1", "DL1:PC2", "DL1:VID",
                "DL1:HD1", "DL1:HD2", "DL1:SVD"
            ])
        );
        let mut m = tcp_module(false, "");
        m.inputs = model_inputs("pt-rz120");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(state(&cx.take()).get("inputs").is_none());
        assert!(model_inputs("pt-unknown").is_empty());
    }

    #[test]
    fn the_command_table_is_sorted() {
        for pair in COMMANDS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn opened_for_commands_only_it_polls_only_the_power() {
        let mut m = tcp_module(false, "");
        m.powered = true;
        m.queue_cycle(false);
        assert_eq!(m.polls.len(), 1);
        let mut m = tcp_module(true, "");
        m.powered = true;
        m.queue_cycle(false);
        assert_eq!(m.polls.len(), 2);
    }
}
