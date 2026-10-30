//! Blackmagic Ultimatte 12 preset archive and restore, added to the
//! spec-driven keyer (`specs/blackmagic-ultimatte.yaml`).
//!
//! Every control, file and network command is the spec's, run by the spec
//! engine on TCP 9998. This adds the unit's binary settings channel on TCP
//! 9996, which Ultimatte Smart Remote 4 and Software Control use for Archive
//! All and Restore. The manual does not document it; the wire is bmdwire's
//! ultimattewire (MIT), reverse-engineered from the vendor app against an
//! Ultimatte 12 4K:
//!
//! - Each request is its own TCP connection, which the unit closes after
//!   answering.
//! - A read is `[opcode u16][length u32][name]`, all big-endian, answered
//!   `[length u32][body]`. Opcode 0x0000 reads a preset by its FILE LIST
//!   name, 0x0003 the `GPISettings` or `SavedSettings` resource.
//! - A write is `[opcode u16][name length u32][name][data length u32][data]`
//!   and a zero byte, answered with six zero bytes (the unit may close after
//!   fewer); any other byte is a refusal. Opcode 0x0100 writes a preset,
//!   0x0103 a resource. The name is the vendor app's path to the member it
//!   extracted, of which the unit reads the last segment and `presets/`.
//! - Archive All's zip holds `images/`, `GPISettings`, `SavedSettings`,
//!   `quickfiles/`, `presets/` and `presets/<name>` for each preset, in that
//!   order; Restore writes the presets in name order, then `GPISettings`,
//!   then `SavedSettings`, which changes the live key, and stops at the first
//!   refusal.
//!
//! The preset names come from the FILE LIST block the unit sends on 9998
//! when connecting and after each file change, read here from the engine's
//! own connection as it passes. These commands work whether or not the
//! device is monitored, since they add commands rather than state.

use std::collections::VecDeque;
use std::net::SocketAddr;

use crate::{json, value::Value};

use super::ultimatte_zip as zip;
use crate::catalog::Params;
use crate::engine::framing::{Framer, ReplyFraming};
use crate::engine::SpecEngine;
use crate::module::{
    CommandError, CommandId, Cx, FileInput, HttpResponse, Key, Module, OpenContext, Outcome,
    RequestId, SettingsUpdate, SseInput, TcpInput, WsInput,
};

const XFER: Key = "ultimatte-settings";
const XFER_TIMEOUT: Key = "ultimatte-settings-timeout";
const XFER_RETRY: Key = "ultimatte-settings-retry";
const READ_FILE: Key = "ultimatte-archive-read";
const WRITE_FILE: Key = "ultimatte-archive-write";

const DEFAULT_PORT: u16 = 9996;
/// bmdwire waits 30 s for an answer; a write of a large preset takes time.
const ANSWER_WITHIN: u64 = 30_000;
/// A cold unit often refuses or drops the first connection and answers the
/// next one.
const RETRY_AFTER: u64 = 250;
/// No preset or resource comes near this; a larger length is not a reply
/// (a frame guard against a broken stream, not a limit on host files).
const MAX_BODY: usize = 64 << 20;

const OP_READ_PRESET: u16 = 0x0000;
const OP_READ_RESOURCE: u16 = 0x0003;
const OP_WRITE_PRESET: u16 = 0x0100;
const OP_WRITE_RESOURCE: u16 = 0x0103;
/// The vendor app's name field: the macOS temporary folder it extracts an
/// archive into. The unit reads the last segment and the `presets/` before
/// it; the rest is sent as the app sends it.
const WIRE_PREFIX: &str =
    "/private/var/folders/00/000000000000000000000000000000/T/Ultimatte Software Control-PYREST";
const RESOURCES: [&str; 2] = ["GPISettings", "SavedSettings"];

/// What the extension answers itself; every other command is the engine's.
const COMMANDS: [&str; 6] = [
    "export_presets",
    "import_presets",
    "export_preset",
    "import_preset",
    "export_settings",
    "import_settings",
];

#[derive(Debug, Clone)]
struct Op {
    /// What the result names it: a preset's name, or a resource's.
    label: String,
    preset: bool,
    opcode: u16,
    /// The read's name, or the write's name field.
    name: String,
    /// A write's data.
    data: Option<Vec<u8>>,
}

impl Op {
    fn read_preset(name: &str) -> Op {
        Op {
            label: name.into(),
            preset: true,
            opcode: OP_READ_PRESET,
            name: name.into(),
            data: None,
        }
    }

    fn read_resource(name: &str) -> Op {
        Op {
            label: name.into(),
            preset: false,
            opcode: OP_READ_RESOURCE,
            name: name.into(),
            data: None,
        }
    }

    fn write_preset(name: &str, data: Vec<u8>) -> Op {
        Op {
            label: name.into(),
            preset: true,
            opcode: OP_WRITE_PRESET,
            name: format!("{WIRE_PREFIX}/presets/{name}"),
            data: Some(data),
        }
    }

    fn write_resource(name: &str, data: Vec<u8>) -> Op {
        Op {
            label: name.into(),
            preset: false,
            opcode: OP_WRITE_RESOURCE,
            name: format!("{WIRE_PREFIX}/{name}"),
            data: Some(data),
        }
    }

    fn frame(&self) -> Vec<u8> {
        let name = self.name.as_bytes();
        let mut out = self.opcode.to_be_bytes().to_vec();
        out.extend_from_slice(&(name.len() as u32).to_be_bytes());
        out.extend_from_slice(name);
        if let Some(data) = &self.data {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            out.push(0);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// Every preset and both resources, into a zip on the host.
    ExportArchive,
    /// A zip's presets and resources, onto the unit.
    ImportArchive,
    /// One preset or resource, into a host file.
    ExportOne,
    /// One host file, onto the unit as a preset or resource.
    ImportOne,
}

#[derive(Debug)]
struct Job {
    id: CommandId,
    kind: Kind,
    path: String,
    /// The single preset or resource of `ExportOne` and `ImportOne`, and
    /// whether it is a preset.
    target: Option<(String, bool)>,
    ops: VecDeque<Op>,
    /// Each op's outcome, in order.
    done: Vec<(Op, Result<Vec<u8>, String>)>,
    /// The archive or file bytes once written, for the result.
    written: usize,
}

/// One request on the settings channel, while it runs.
#[derive(Debug, Default)]
struct Exchange {
    connected: bool,
    retried: bool,
    received: Vec<u8>,
}

pub(crate) struct UltimattePresets {
    engine: SpecEngine,
    to: SocketAddr,
    /// FILE LIST as the unit last sent it on 9998; `None` until it has.
    files: Option<Vec<String>>,
    blocks: Framer,
    jobs: VecDeque<Job>,
    exchange: Option<Exchange>,
    /// The current job is reading or writing its host file.
    on_file: bool,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn refused(message: impl Into<String>) -> CommandError {
    CommandError::DeviceError {
        code: None,
        message: message.into(),
    }
}

fn port_of(settings: &Params) -> u16 {
    settings
        .get("archive_port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|&p| p != 0)
        .unwrap_or(DEFAULT_PORT)
}

/// A FILE LIST block's names: the lines after its header.
fn file_list(block: &str) -> Option<Vec<String>> {
    let mut lines = block.lines();
    if lines.next()?.trim() != "FILE LIST:" {
        return None;
    }
    Some(
        lines
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

impl UltimattePresets {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> UltimattePresets {
        UltimattePresets {
            engine,
            to: SocketAddr::new(ctx.host, port_of(&ctx.settings)),
            files: None,
            blocks: Framer::new(ReplyFraming::Block),
            jobs: VecDeque::new(),
            exchange: None,
            on_file: false,
        }
    }

    fn param<'a>(params: &'a Params, name: &str) -> &'a str {
        params.get(name).and_then(Value::as_str).unwrap_or("")
    }

    fn plan(&self, id: CommandId, name: &str, params: &Params) -> Result<Job, CommandError> {
        let path = Self::param(params, "path").to_string();
        if path.is_empty() {
            return Err(invalid("'path' is empty"));
        }
        let mut job = Job {
            id,
            kind: Kind::ExportArchive,
            path,
            target: None,
            ops: VecDeque::new(),
            done: Vec::new(),
            written: 0,
        };
        match name {
            "export_presets" => {
                let Some(files) = &self.files else {
                    return Err(CommandError::NotConnected);
                };
                job.ops.extend(files.iter().map(|f| Op::read_preset(f)));
                job.ops
                    .extend(RESOURCES.iter().map(|r| Op::read_resource(r)));
            }
            "import_presets" => job.kind = Kind::ImportArchive,
            "export_preset" | "import_preset" => {
                let preset = Self::param(params, "name").to_string();
                job.kind = if name == "export_preset" {
                    Kind::ExportOne
                } else {
                    Kind::ImportOne
                };
                if job.kind == Kind::ExportOne {
                    job.ops.push_back(Op::read_preset(&preset));
                }
                job.target = Some((preset, true));
            }
            _ => {
                let resource = Self::param(params, "resource").to_string();
                if !RESOURCES.contains(&resource.as_str()) {
                    return Err(invalid("'resource' is GPISettings or SavedSettings"));
                }
                job.kind = if name == "export_settings" {
                    Kind::ExportOne
                } else {
                    Kind::ImportOne
                };
                if job.kind == Kind::ExportOne {
                    job.ops.push_back(Op::read_resource(&resource));
                }
                job.target = Some((resource, false));
            }
        }
        Ok(job)
    }

    /// Start the job at the head of the queue, if nothing runs.
    fn pump(&mut self, cx: &mut Cx) {
        if self.exchange.is_some() || self.on_file {
            return;
        }
        let Some(job) = self.jobs.front_mut() else {
            return;
        };
        if matches!(job.kind, Kind::ImportArchive | Kind::ImportOne)
            && job.done.is_empty()
            && job.ops.is_empty()
        {
            self.on_file = true;
            cx.file_read(READ_FILE, job.path.clone());
            return;
        }
        if job.ops.is_empty() {
            self.finish(cx);
            return;
        }
        self.exchange = Some(Exchange::default());
        cx.tcp_open(XFER, self.to);
        cx.set_timer(XFER_TIMEOUT, ANSWER_WITHIN);
    }

    /// The host file of an import has been read: plan its writes.
    fn file_read(&mut self, cx: &mut Cx, data: Vec<u8>) {
        let Some(job) = self.jobs.front_mut() else {
            return;
        };
        let ops = match job.kind {
            Kind::ImportArchive => restore_ops(&data),
            _ => {
                let (name, preset) = job.target.clone().unwrap_or_default();
                if data.is_empty() {
                    Err("the file is empty".to_string())
                } else if preset {
                    Ok(vec![Op::write_preset(&name, data)])
                } else {
                    Ok(vec![Op::write_resource(&name, data)])
                }
            }
        };
        match ops {
            Ok(ops) => {
                job.ops.extend(ops);
                self.pump(cx);
            }
            Err(e) => self.fail(cx, invalid(e)),
        }
    }

    /// The current job fails; the next one starts.
    fn fail(&mut self, cx: &mut Cx, error: CommandError) {
        if let Some(job) = self.jobs.pop_front() {
            cx.complete(job.id, Err(error));
        }
        self.on_file = false;
        self.pump(cx);
    }

    /// The current request is answered (or failed): note it, go on.
    fn answered(&mut self, cx: &mut Cx, result: Result<Vec<u8>, String>) {
        self.exchange = None;
        cx.cancel_timer(XFER_TIMEOUT);
        cx.cancel_timer(XFER_RETRY);
        cx.tcp_close(XFER);
        let Some(job) = self.jobs.front_mut() else {
            return;
        };
        let Some(op) = job.ops.pop_front() else {
            return;
        };
        let writing = op.data.is_some();
        if let (true, Err(e)) = (writing, &result) {
            // A restore stops at the first refusal, as the vendor app's does.
            let wrote: Vec<&str> = job.done.iter().map(|(o, _)| o.label.as_str()).collect();
            let message = if wrote.is_empty() {
                format!("the unit refused {}: {e}", op.label)
            } else {
                format!(
                    "the unit refused {}: {e}; written before it: {}",
                    op.label,
                    wrote.join(", ")
                )
            };
            return self.fail(cx, refused(message));
        }
        job.done.push((op, result));
        self.pump(cx);
    }

    /// Every request of the current job has been answered.
    fn finish(&mut self, cx: &mut Cx) {
        let Some(job) = self.jobs.front_mut() else {
            return;
        };
        match job.kind {
            Kind::ImportArchive | Kind::ImportOne => {
                let job = self.jobs.pop_front().unwrap();
                let outcome = if job.kind == Kind::ImportOne {
                    Outcome::Ack
                } else {
                    let names = |preset: bool| -> Vec<&str> {
                        job.done
                            .iter()
                            .filter(|(o, _)| o.preset == preset)
                            .map(|(o, _)| o.label.as_str())
                            .collect()
                    };
                    Outcome::Value {
                        value: json!({ "presets": names(true), "settings": names(false) }),
                    }
                };
                cx.complete(job.id, Ok(outcome));
                self.pump(cx);
            }
            Kind::ExportOne => {
                let (op, result) = job.done.pop().expect("one read");
                match result {
                    Ok(data) => {
                        job.written = data.len();
                        self.write_file(cx, data);
                    }
                    Err(e) => self.fail(cx, refused(format!("reading {}: {e}", op.label))),
                }
            }
            Kind::ExportArchive => {
                if job.done.iter().all(|(_, r)| r.is_err()) {
                    let reason = job
                        .done
                        .first()
                        .and_then(|(_, r)| r.as_ref().err().cloned())
                        .unwrap_or_default();
                    return self.fail(
                        cx,
                        CommandError::Transport {
                            message: format!(
                                "the unit's settings channel answered nothing: {reason}"
                            ),
                        },
                    );
                }
                let archive = archive(&job.done, cx.unix_millis());
                job.written = archive.len();
                self.write_file(cx, archive);
            }
        }
    }

    fn write_file(&mut self, cx: &mut Cx, data: Vec<u8>) {
        let Some(job) = self.jobs.front() else {
            return;
        };
        self.on_file = true;
        cx.file_open(WRITE_FILE, job.path.clone());
        cx.file_write(WRITE_FILE, data);
        cx.file_close(WRITE_FILE);
    }

    /// The export's host file is written: the command's result.
    fn file_written(&mut self, cx: &mut Cx) {
        self.on_file = false;
        let Some(job) = self.jobs.pop_front() else {
            return;
        };
        let value = match job.kind {
            Kind::ExportArchive => {
                let names = |preset: bool| -> Vec<&str> {
                    job.done
                        .iter()
                        .filter(|(o, r)| o.preset == preset && r.is_ok())
                        .map(|(o, _)| o.label.as_str())
                        .collect()
                };
                let warnings: Vec<String> = job
                    .done
                    .iter()
                    .filter_map(|(o, r)| r.as_ref().err().map(|e| format!("{}: {e}", o.label)))
                    .collect();
                json!({
                    "path": job.path,
                    "bytes": job.written,
                    "presets": names(true),
                    "settings": names(false),
                    "warnings": warnings,
                })
            }
            _ => json!({ "path": job.path, "bytes": job.written }),
        };
        cx.complete(job.id, Ok(Outcome::Value { value }));
        self.pump(cx);
    }

    /// What has arrived on the settings channel answers the request when
    /// it is whole.
    fn received(&mut self, cx: &mut Cx) {
        let Some(exchange) = &self.exchange else {
            return;
        };
        let writing = self
            .jobs
            .front()
            .and_then(|j| j.ops.front())
            .is_some_and(|o| o.data.is_some());
        let got = &exchange.received;
        if writing {
            if got.iter().any(|&b| b != 0) {
                let hex: String = got.iter().map(|b| format!("{b:02x}")).collect();
                return self.answered(cx, Err(format!("the answer was {hex}, not zero bytes")));
            }
            if got.len() >= 6 {
                self.answered(cx, Ok(Vec::new()));
            }
            return;
        }
        if got.len() < 4 {
            return;
        }
        let length = u32::from_be_bytes(got[..4].try_into().unwrap()) as usize;
        if length > MAX_BODY {
            return self.answered(cx, Err(format!("the unit announced {length} bytes")));
        }
        if got.len() >= 4 + length {
            let body = got[4..4 + length].to_vec();
            self.answered(cx, Ok(body));
        }
    }

    /// The unit closed the settings channel.
    fn closed(&mut self, cx: &mut Cx, reason: &str) {
        let writing = self
            .jobs
            .front()
            .and_then(|j| j.ops.front())
            .is_some_and(|o| o.data.is_some());
        let Some(exchange) = &mut self.exchange else {
            return;
        };
        // A cold unit refuses the first connection, or drops it unanswered.
        // A connection that never opened is tried again, and so is a read
        // that got nothing; a write is not, since it may have been taken.
        let unanswered_read = !writing && exchange.received.is_empty();
        if !exchange.retried && (!exchange.connected || unanswered_read) {
            *exchange = Exchange {
                retried: true,
                ..Exchange::default()
            };
            cx.set_timer(XFER_RETRY, RETRY_AFTER);
            return;
        }
        let got = exchange.received.len();
        let result = if !exchange.connected {
            Err(format!("cannot connect: {reason}"))
        } else if writing && got > 0 {
            // The unit closes so soon after its acknowledgement that fewer
            // than six bytes may arrive; all zero, it is one.
            Ok(Vec::new())
        } else if writing {
            Err("the unit closed the connection without acknowledging".into())
        } else {
            Err(format!("the unit closed the connection after {got} bytes"))
        };
        self.answered(cx, result);
    }

    fn tcp_xfer(&mut self, cx: &mut Cx, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                let frame = self.jobs.front().and_then(|j| j.ops.front()).map(Op::frame);
                if let (Some(exchange), Some(frame)) = (self.exchange.as_mut(), frame) {
                    exchange.connected = true;
                    cx.tcp_send(XFER, frame);
                }
            }
            TcpInput::Data(data) => {
                if let Some(exchange) = self.exchange.as_mut() {
                    exchange.received.extend_from_slice(&data);
                    self.received(cx);
                }
            }
            TcpInput::Closed { reason } => self.closed(cx, &reason),
        }
    }
}

/// Archive All's zip, in the vendor app's order; a member whose read failed
/// is left out rather than written empty, so a restore never sends it.
fn archive(done: &[(Op, Result<Vec<u8>, String>)], unix_millis: u64) -> Vec<u8> {
    let read = |preset: bool, label: &str| {
        done.iter()
            .find(|(o, r)| o.preset == preset && o.label == label && r.is_ok())
            .and_then(|(_, r)| r.as_ref().ok())
    };
    let presets: Vec<(String, &Vec<u8>)> = done
        .iter()
        .filter(|(o, _)| o.preset)
        .filter_map(|(o, r)| r.as_ref().ok().map(|d| (format!("presets/{}", o.label), d)))
        .collect();
    let mut members = vec![zip::Member {
        name: "images/",
        data: b"",
    }];
    for resource in RESOURCES {
        if let Some(data) = read(false, resource) {
            members.push(zip::Member {
                name: resource,
                data,
            });
        }
    }
    members.push(zip::Member {
        name: "quickfiles/",
        data: b"",
    });
    members.push(zip::Member {
        name: "presets/",
        data: b"",
    });
    for (name, data) in &presets {
        members.push(zip::Member { name, data });
    }
    zip::write(&members, unix_millis)
}

/// Restore's writes from an archive: its presets in name order, then
/// GPISettings, then SavedSettings. Other members (a text dump, the macOS
/// metadata a Finder zip adds) are not sent.
fn restore_ops(data: &[u8]) -> Result<Vec<Op>, String> {
    let members = zip::read(data)?;
    let mut presets: Vec<(&str, &Vec<u8>)> = members
        .iter()
        .filter_map(|(name, data)| {
            let preset = name.strip_prefix("presets/")?;
            (!preset.is_empty() && !preset.contains('/')).then_some((preset, data))
        })
        .collect();
    presets.sort_by(|a, b| a.0.cmp(b.0));
    let mut ops: Vec<Op> = presets
        .into_iter()
        .map(|(name, data)| Op::write_preset(name, data.clone()))
        .collect();
    for resource in RESOURCES {
        if let Some((_, data)) = members.iter().find(|(n, _)| n == resource) {
            ops.push(Op::write_resource(resource, data.clone()));
        }
    }
    if ops.is_empty() {
        return Err("the archive holds no presets, GPISettings or SavedSettings".into());
    }
    Ok(ops)
}

impl Module for UltimattePresets {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !COMMANDS.contains(&name) {
            return self.engine.command(cx, id, name, params);
        }
        match self.plan(id, name, params) {
            Ok(job) => {
                self.jobs.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket == XFER {
            return self.tcp_xfer(cx, input);
        }
        // The engine's control connection: keep the unit's FILE LIST.
        match &input {
            TcpInput::Data(data) => {
                for block in self.blocks.feed(data) {
                    if let Some(files) = file_list(&block) {
                        self.files = Some(files);
                    }
                }
            }
            TcpInput::Connected | TcpInput::Closed { .. } => {
                self.blocks = Framer::new(ReplyFraming::Block);
                self.files = None;
            }
        }
        self.engine.tcp(cx, socket, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.engine.http_response(cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        match (file, input) {
            (READ_FILE, FileInput::Read { data }) => {
                self.on_file = false;
                self.file_read(cx, data);
            }
            (WRITE_FILE, FileInput::Closed { .. }) => self.file_written(cx),
            (READ_FILE | WRITE_FILE, FileInput::Failed { message }) => {
                let path = self
                    .jobs
                    .front()
                    .map(|j| j.path.clone())
                    .unwrap_or_default();
                self.fail(
                    cx,
                    CommandError::Transport {
                        message: format!("host file {path}: {message}"),
                    },
                );
            }
            (READ_FILE | WRITE_FILE, _) => {}
            (other, input) => self.engine.file(cx, other, input),
        }
    }

    fn computed(&mut self, cx: &mut Cx, key: Key, result: Result<Vec<u8>, String>) {
        self.engine.computed(cx, key, result);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            XFER_TIMEOUT => {
                if self.exchange.is_some() {
                    self.answered(cx, Err("no answer within 30 s".into()));
                }
            }
            XFER_RETRY => {
                if self.exchange.is_some() {
                    cx.tcp_open(XFER, self.to);
                }
            }
            _ => self.engine.timer(cx, key),
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(XFER);
        self.engine.stop(cx);
    }

    fn update_settings(&mut self, cx: &mut Cx, settings: &Params) -> SettingsUpdate {
        let update = self.engine.update_settings(cx, settings);
        if update == SettingsUpdate::Applied {
            self.to.set_port(port_of(settings));
        }
        update
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        self.engine.reconnect(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_the_vendor_apps() {
        let read = Op::read_resource("GPISettings").frame();
        assert_eq!(&read[..6], &[0x00, 0x03, 0, 0, 0, 11]);
        assert_eq!(&read[6..], b"GPISettings");

        let write = Op::write_preset("Studio A", b"blob".to_vec()).frame();
        let name = format!("{WIRE_PREFIX}/presets/Studio A");
        assert_eq!(&write[..2], &[0x01, 0x00]);
        assert_eq!(&write[2..6], &(name.len() as u32).to_be_bytes());
        assert_eq!(&write[6..6 + name.len()], name.as_bytes());
        assert_eq!(
            &write[6 + name.len()..],
            &[0, 0, 0, 4, b'b', b'l', b'o', b'b', 0]
        );
    }

    #[test]
    fn file_list_blocks_give_names() {
        assert_eq!(
            file_list("FILE LIST:\nGreen Set\nBlue Set"),
            Some(vec!["Green Set".into(), "Blue Set".into()])
        );
        assert_eq!(file_list("FILE LIST:"), Some(vec![]));
        assert_eq!(file_list("CURRENT FILE:\nName: x"), None);
    }

    #[test]
    fn archives_restore_in_the_vendor_apps_order() {
        let done = vec![
            (Op::read_preset("b"), Ok(b"B".to_vec())),
            (Op::read_preset("a"), Ok(b"A".to_vec())),
            (Op::read_preset("lost"), Err("short".into())),
            (Op::read_resource("GPISettings"), Ok(b"G".to_vec())),
            (Op::read_resource("SavedSettings"), Ok(b"S".to_vec())),
        ];
        let zip = archive(&done, 0);
        let names: Vec<String> = zip::read(&zip)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            ["GPISettings", "SavedSettings", "presets/b", "presets/a"]
        );

        let ops = restore_ops(&zip).unwrap();
        let order: Vec<(&str, u16)> = ops.iter().map(|o| (o.label.as_str(), o.opcode)).collect();
        assert_eq!(
            order,
            [
                ("a", OP_WRITE_PRESET),
                ("b", OP_WRITE_PRESET),
                ("GPISettings", OP_WRITE_RESOURCE),
                ("SavedSettings", OP_WRITE_RESOURCE)
            ]
        );
        assert_eq!(ops[0].data.as_deref(), Some(&b"A"[..]));
    }

    #[test]
    fn an_archive_with_nothing_to_restore_is_refused() {
        let zip = zip::write(
            &[zip::Member {
                name: "notes.txt",
                data: b"x",
            }],
            0,
        );
        assert!(restore_ops(&zip).unwrap_err().contains("no presets"));
    }
}
