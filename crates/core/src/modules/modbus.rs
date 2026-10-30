//! A generic Modbus TCP client for one server (unit): reads coils,
//! discrete inputs, holding and input registers into state from a poll
//! list, and writes coils and registers, as the MODBUS Application Protocol
//! Specification V1.1b3 and the MODBUS Messaging on TCP/IP Implementation
//! Guide V1.0b (modbus.org) define them.
//!
//! - Each request is an MBAP header (transaction id, protocol 0, length,
//!   unit id) and a PDU (function code and data). One request is
//!   outstanding at a time; its response is matched by transaction id, and
//!   an exception response (function code + 80h) fails it with the
//!   exception code. Commands go ahead of polls.
//! - Every entry of `poll` is read every `poll_interval_ms`; the raw
//!   tables of the device's unit are kept in state by 0-based protocol
//!   address (`coils.<address>`, `holding_registers.<address>`, ...), and
//!   a named entry's decoded value in `values.<name>`. Reads and writes of
//!   the device's unit update the tables too, and a write is followed by a
//!   read of the polled entries it touched.
//! - The device identification (function 43/14) is read on connecting;
//!   a server without it answers an exception, which is ignored.
//! - The server is connected while it answers; three requests in a row
//!   unanswered, or a connection that drops, close the connection and open
//!   it again after 1 s, doubling to 30 s.
//!
//! Opened for commands only (`monitor` false), nothing is polled and the
//! identification is not read; one read of the holding register at
//! `liveness_address` each `poll_interval_ms` of silence stays as the
//! liveness check (any answer, an exception included, shows the server is
//! there). Every answered request reports its round trip as the latency.

use std::collections::VecDeque;
use std::net::SocketAddr;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    PollStamps, TcpInput,
};

pub(crate) const PORT: u16 = 502;
const SOCKET: Key = "modbus";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const RETRY_TIMER: Key = "retry";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Requests unanswered in a row before the connection is dropped.
const MISSES: u32 = 3;

const READ_COILS: u8 = 0x01;
const READ_DISCRETE_INPUTS: u8 = 0x02;
const READ_HOLDING_REGISTERS: u8 = 0x03;
const READ_INPUT_REGISTERS: u8 = 0x04;
const WRITE_SINGLE_COIL: u8 = 0x05;
const WRITE_SINGLE_REGISTER: u8 = 0x06;
const WRITE_MULTIPLE_COILS: u8 = 0x0F;
const WRITE_MULTIPLE_REGISTERS: u8 = 0x10;
const READ_FILE_RECORD: u8 = 0x14;
const WRITE_FILE_RECORD: u8 = 0x15;
const MASK_WRITE_REGISTER: u8 = 0x16;
const READ_WRITE_MULTIPLE_REGISTERS: u8 = 0x17;
const READ_FIFO_QUEUE: u8 = 0x18;
const ENCAPSULATED_INTERFACE: u8 = 0x2B;
const READ_DEVICE_IDENTIFICATION: u8 = 0x0E;

/// Quantity limits of the specification (§6).
const MAX_READ_BITS: u16 = 2000;
const MAX_READ_REGISTERS: u16 = 125;
const MAX_WRITE_BITS: u16 = 1968;
const MAX_WRITE_REGISTERS: u16 = 123;
const MAX_RW_WRITE_REGISTERS: u16 = 121;
/// File records: record numbers 0 to 9999 (§6.14).
const MAX_RECORD: u16 = 9999;

/// The exception codes (§7).
fn exception_text(code: u8) -> &'static str {
    match code {
        1 => "illegal function",
        2 => "illegal data address",
        3 => "illegal data value",
        4 => "server device failure",
        5 => "acknowledge (the request will take long to complete)",
        6 => "server device busy",
        8 => "memory parity error",
        0x0A => "gateway path unavailable",
        0x0B => "gateway target device failed to respond",
        _ => "unknown exception",
    }
}

/// A data table of the data model (§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Table {
    Coils,
    DiscreteInputs,
    HoldingRegisters,
    InputRegisters,
}

impl Table {
    fn parse(text: &str) -> Option<Table> {
        match text.to_ascii_lowercase().as_str() {
            "co" | "coil" | "coils" => Some(Table::Coils),
            "di" | "discrete" | "discrete_inputs" => Some(Table::DiscreteInputs),
            "hr" | "holding" | "holding_registers" => Some(Table::HoldingRegisters),
            "ir" | "input" | "input_registers" => Some(Table::InputRegisters),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Table::Coils => "coils",
            Table::DiscreteInputs => "discrete_inputs",
            Table::HoldingRegisters => "holding_registers",
            Table::InputRegisters => "input_registers",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Table::Coils => "co",
            Table::DiscreteInputs => "di",
            Table::HoldingRegisters => "hr",
            Table::InputRegisters => "ir",
        }
    }

    fn read_function(self) -> u8 {
        match self {
            Table::Coils => READ_COILS,
            Table::DiscreteInputs => READ_DISCRETE_INPUTS,
            Table::HoldingRegisters => READ_HOLDING_REGISTERS,
            Table::InputRegisters => READ_INPUT_REGISTERS,
        }
    }

    fn bits(self) -> bool {
        matches!(self, Table::Coils | Table::DiscreteInputs)
    }

    fn max_read(self) -> u16 {
        if self.bits() {
            MAX_READ_BITS
        } else {
            MAX_READ_REGISTERS
        }
    }
}

/// How a run of registers is read as a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    U16,
    I16,
    U32,
    I32,
    F32,
    U64,
    I64,
    F64,
    /// Two characters per register, high byte first, trailing NULs dropped.
    String,
}

impl Format {
    fn parse(text: &str) -> Option<Format> {
        match text.to_ascii_lowercase().as_str() {
            "u16" | "uint16" => Some(Format::U16),
            "i16" | "int16" => Some(Format::I16),
            "u32" | "uint32" => Some(Format::U32),
            "i32" | "int32" => Some(Format::I32),
            "f32" | "float32" | "float" => Some(Format::F32),
            "u64" | "uint64" => Some(Format::U64),
            "i64" | "int64" => Some(Format::I64),
            "f64" | "float64" | "double" => Some(Format::F64),
            "string" | "str" | "text" => Some(Format::String),
            _ => None,
        }
    }

    /// Registers one value takes; `None` for a string, which takes the
    /// entry's count.
    fn width(self) -> Option<u16> {
        match self {
            Format::U16 | Format::I16 => Some(1),
            Format::U32 | Format::I32 | Format::F32 => Some(2),
            Format::U64 | Format::I64 | Format::F64 => Some(4),
            Format::String => None,
        }
    }
}

/// The order of the 16-bit words of a value wider than one register. The
/// specification sends each register high byte first and leaves wider
/// values to the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordOrder {
    /// The most significant word at the lowest address.
    Big,
    /// The least significant word at the lowest address.
    Little,
}

/// A run of registers as one value, or a run of values.
pub(crate) fn decode(registers: &[u16], format: Format, order: WordOrder) -> Value {
    if format == Format::String {
        let mut bytes: Vec<u8> = registers.iter().flat_map(|r| r.to_be_bytes()).collect();
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        return Value::String(String::from_utf8_lossy(&bytes).into_owned());
    }
    let width = format.width().unwrap_or(1) as usize;
    let values: Vec<Value> = registers
        .chunks(width)
        .filter(|c| c.len() == width)
        .map(|chunk| {
            let mut words = chunk.to_vec();
            if order == WordOrder::Little {
                words.reverse();
            }
            let raw = words.iter().fold(0u64, |a, &w| (a << 16) | u64::from(w));
            match format {
                Format::U16 | Format::U32 | Format::U64 => json!(raw),
                Format::I16 => json!(raw as u16 as i16),
                Format::I32 => json!(raw as u32 as i32),
                Format::I64 => json!(raw as i64),
                Format::F32 => float(f64::from(f32::from_bits(raw as u32))),
                Format::F64 => float(f64::from_bits(raw)),
                Format::String => Value::Null,
            }
        })
        .collect();
    match values.len() {
        1 => values.into_iter().next().unwrap_or(Value::Null),
        _ => Value::Array(values),
    }
}

/// A float as JSON; NaN and the infinities, which JSON cannot hold, as
/// their names.
fn float(f: f64) -> Value {
    if f.is_finite() {
        json!(f)
    } else if f.is_nan() {
        json!("nan")
    } else if f > 0.0 {
        json!("inf")
    } else {
        json!("-inf")
    }
}

/// A value as registers, for writing. `None` when it does not fit.
pub(crate) fn encode(value: f64, format: Format, order: WordOrder) -> Option<Vec<u16>> {
    let fits = |lo: f64, hi: f64| value.fract() == 0.0 && value >= lo && value <= hi;
    let (raw, width): (u64, usize) = match format {
        Format::U16 if fits(0.0, 65535.0) => (value as u64, 1),
        Format::I16 if fits(-32768.0, 32767.0) => (value as i16 as u16 as u64, 1),
        Format::U32 if fits(0.0, 4_294_967_295.0) => (value as u64, 2),
        Format::I32 if fits(-2_147_483_648.0, 2_147_483_647.0) => (value as i32 as u32 as u64, 2),
        Format::F32 if value.is_finite() => (u64::from((value as f32).to_bits()), 2),
        Format::U64 if fits(0.0, 18_446_744_073_709_551_615.0) => (value as u64, 4),
        Format::I64 if fits(-9_223_372_036_854_775_808.0, 9_223_372_036_854_775_807.0) => {
            (value as i64 as u64, 4)
        }
        Format::F64 if value.is_finite() => (value.to_bits(), 4),
        _ => return None,
    };
    let mut words: Vec<u16> = (0..width).rev().map(|i| (raw >> (16 * i)) as u16).collect();
    if order == WordOrder::Little {
        words.reverse();
    }
    Some(words)
}

/// Bits packed LSB first, as coils travel (§6.1, §6.11).
fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

fn unpack_bits(bytes: &[u8], count: usize) -> Vec<bool> {
    (0..count)
        .map(|i| bytes.get(i / 8).is_some_and(|b| b & (1 << (i % 8)) != 0))
        .collect()
}

fn words(bytes: &[u8]) -> Vec<u16> {
    let (pairs, _) = bytes.as_chunks::<2>();
    pairs.iter().map(|&c| u16::from_be_bytes(c)).collect()
}

fn word_bytes(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// An MBAP header and PDU (Messaging Implementation Guide §3.1.3).
pub(crate) fn adu(transaction: u16, unit: u8, pdu: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pdu.len() + 7);
    out.extend_from_slice(&transaction.to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    out.push(unit);
    out.extend_from_slice(pdu);
    out
}

/// Splits the stream into (transaction, unit, PDU).
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<(u16, u8, Vec<u8>)> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while self.buf.len() >= 7 {
            let len = usize::from(u16::from_be_bytes([self.buf[4], self.buf[5]]));
            if !(2..=254).contains(&len) || self.buf[2] != 0 || self.buf[3] != 0 {
                // Not a Modbus header: the stream is lost; drop it.
                self.buf.clear();
                break;
            }
            if self.buf.len() < 6 + len {
                break;
            }
            let transaction = u16::from_be_bytes([self.buf[0], self.buf[1]]);
            let unit = self.buf[6];
            let pdu = self.buf[7..6 + len].to_vec();
            self.buf.drain(..6 + len);
            out.push((transaction, unit, pdu));
        }
        out
    }
}

/// One entry of the poll list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub name: Option<String>,
    pub table: Table,
    pub address: u16,
    pub count: u16,
    pub format: Format,
}

impl Entry {
    fn label(&self) -> String {
        match &self.name {
            Some(n) => n.clone(),
            None => format!("{}:{}", self.table.short(), self.address),
        }
    }

    fn end(&self) -> u32 {
        u32::from(self.address) + u32::from(self.count)
    }

    fn overlaps(&self, table: Table, address: u16, count: u16) -> bool {
        self.table == table
            && u32::from(address) < self.end()
            && u32::from(self.address) < u32::from(address) + u32::from(count)
    }
}

/// The `poll` setting: entries `[name=]table:address[:count][:format]`,
/// separated by commas, semicolons or white space.
pub(crate) fn parse_poll(text: &str) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for item in text
        .split([',', ';', ' ', '\n', '\t', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (name, spec) = match item.split_once('=') {
            Some((n, s)) => {
                let n = n.trim();
                if n.is_empty()
                    || !n
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(format!(
                        "poll entry '{item}': a name is letters, digits, _ and -"
                    ));
                }
                (Some(n.to_string()), s)
            }
            None => (None, item),
        };
        let parts: Vec<&str> = spec.split(':').map(str::trim).collect();
        let table = parts.first().and_then(|t| Table::parse(t)).ok_or(format!(
            "poll entry '{item}': the table is co, di, hr or ir"
        ))?;
        let address: u16 = parts
            .get(1)
            .and_then(|a| a.parse().ok())
            .ok_or(format!("poll entry '{item}': the address is 0 to 65535"))?;
        let mut count: Option<u16> = None;
        let mut format: Option<Format> = None;
        for part in parts.iter().skip(2) {
            if let Ok(n) = part.parse::<u16>() {
                if count.is_some() {
                    return Err(format!("poll entry '{item}' gives two counts"));
                }
                count = Some(n);
            } else if let Some(f) = Format::parse(part) {
                if table.bits() {
                    return Err(format!(
                        "poll entry '{item}': coils and discrete inputs take no format"
                    ));
                }
                format = Some(f);
            } else {
                return Err(format!(
                    "poll entry '{item}': '{part}' is not a count or a format"
                ));
            }
        }
        let format = format.unwrap_or(Format::U16);
        let count = match (count, format.width()) {
            (Some(c), _) => c,
            (None, Some(w)) => w,
            (None, None) => {
                return Err(format!("poll entry '{item}': a string needs a count"));
            }
        };
        if count == 0 {
            return Err(format!("poll entry '{item}': the count is at least 1"));
        }
        if u32::from(address) + u32::from(count) > 65536 {
            return Err(format!("poll entry '{item}' runs past address 65535"));
        }
        if name.is_some() && count > table.max_read() {
            return Err(format!(
                "poll entry '{item}': a named entry is read in one request, at most {} {}",
                table.max_read(),
                if table.bits() { "bits" } else { "registers" }
            ));
        }
        if let Some(w) = format.width() {
            if count % w != 0 {
                return Err(format!(
                    "poll entry '{item}': the count is a multiple of the format's {w} registers"
                ));
            }
        }
        out.push(Entry {
            name,
            table,
            address,
            count,
            format,
        });
    }
    Ok(out)
}

/// What a request is for.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Command {
        id: CommandId,
        /// How the answer becomes the result.
        result: Answer,
    },
    /// A chunk of a poll entry: the entry's index and the chunk's first
    /// address.
    Poll {
        entry: usize,
    },
    Identification,
    /// A read of what a write left in the device's own table.
    ReadBack(Table),
    Liveness,
}

/// How a command's answer is returned.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Answer {
    /// The bits or registers read, as a list.
    Raw,
    /// The registers read, decoded.
    Decoded(Format),
    /// The written range acknowledged.
    Ack,
    /// The FIFO queue's values.
    Fifo,
    /// A file record's registers.
    Record,
    /// The identification objects.
    Identification,
}

#[derive(Debug, Clone)]
struct Request {
    unit: u8,
    pdu: Vec<u8>,
    purpose: Purpose,
}

pub(crate) struct Modbus {
    to: SocketAddr,
    unit: u8,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    entries: Vec<Entry>,
    poll_every: Millis,
    timeout: Millis,
    order: WordOrder,
    liveness_address: u16,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    transaction: u16,
    deframer: Deframer,
    queue: VecDeque<Request>,
    in_flight: Option<(u16, Request, Millis)>,
    /// When something was last heard, for the liveness check.
    heard_at: Millis,
    /// Identification objects gathered over several responses.
    identification: Map<String, Value>,
}

fn int(s: &Params, key: &str, default: u64) -> u64 {
    s.get(key).and_then(Value::as_u64).unwrap_or(default)
}

impl Modbus {
    pub(crate) fn new(ctx: OpenContext) -> Result<Modbus, String> {
        if ctx.host.is_unspecified() {
            return Err("give the Modbus server's address as host".into());
        }
        let s = &ctx.settings;
        let entries = parse_poll(s.get("poll").and_then(Value::as_str).unwrap_or(""))?;
        let order = match s.get("word_order").and_then(Value::as_str) {
            Some("little") => WordOrder::Little,
            _ => WordOrder::Big,
        };
        Ok(Modbus {
            to: SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)),
            unit: int(s, "unit_id", 1).min(255) as u8,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            entries,
            poll_every: int(s, "poll_interval_ms", 1000).max(10),
            timeout: int(s, "timeout_ms", 2000).max(1),
            order,
            liveness_address: int(s, "liveness_address", 0).min(65535) as u16,
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            transaction: 0,
            deframer: Deframer::default(),
            queue: VecDeque::new(),
            in_flight: None,
            heard_at: 0,
            identification: Map::new(),
        })
    }

    fn connect(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.to);
    }

    /// Queue every chunk of every poll entry not already waiting.
    fn sweep(&mut self) {
        for (i, entry) in self.entries.clone().iter().enumerate() {
            self.queue_entry(i, entry);
        }
    }

    fn queue_entry(&mut self, index: usize, entry: &Entry) {
        let max = entry.table.max_read();
        let mut address = u32::from(entry.address);
        while address < entry.end() {
            let count = (entry.end() - address).min(u32::from(max)) as u16;
            let pdu = read_pdu(entry.table, address as u16, count);
            let waiting = self
                .queue
                .iter()
                .any(|r| r.pdu == pdu && r.unit == self.unit)
                || self
                    .in_flight
                    .as_ref()
                    .is_some_and(|(_, r, _)| r.pdu == pdu && r.unit == self.unit);
            if !waiting {
                self.queue.push_back(Request {
                    unit: self.unit,
                    pdu,
                    purpose: Purpose::Poll { entry: index },
                });
            }
            address += u32::from(count);
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, request: Request) {
        let is_command = matches!(request.purpose, Purpose::Command { .. });
        let at = self
            .queue
            .iter()
            .position(|r| !matches!(r.purpose, Purpose::Command { .. }));
        match (is_command, at) {
            (true, Some(at)) => self.queue.insert(at, request),
            _ => self.queue.push_back(request),
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        self.transaction = self.transaction.wrapping_add(1);
        cx.tcp_send(SOCKET, adu(self.transaction, request.unit, &request.pdu));
        cx.set_timer(REPLY_TIMER, self.timeout);
        self.in_flight = Some((self.transaction, request, cx.now()));
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        self.heard_at = cx.now();
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.deframer = Deframer::default();
        for request in self
            .in_flight
            .take()
            .map(|(_, r, _)| r)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Purpose::Command { id, .. } = request.purpose {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        cx.cancel_timer(REPLY_TIMER);
        cx.cancel_timer(POLL_TIMER);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The raw table values of the device's unit, as a state patch.
    fn table_patch(&self, table: Table, address: u16, values: &[Value]) -> Value {
        let mut rows = Map::new();
        for (i, v) in values.iter().enumerate() {
            rows.insert((u32::from(address) + i as u32).to_string(), v.clone());
        }
        json!({ table.key(): rows })
    }

    /// Re-read the polled entries a write touched.
    fn reread(&mut self, table: Table, address: u16, count: u16) {
        if !self.monitor {
            return;
        }
        for (i, entry) in self.entries.clone().iter().enumerate() {
            if entry.overlaps(table, address, count) {
                self.queue_entry(i, entry);
            }
        }
    }

    /// A poll entry's chunk read: its table values, and a named entry's
    /// decoded value (a named entry is one request).
    fn apply_entry(&self, cx: &mut Cx, index: usize, address: u16, data: &ReadData) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        cx.state(self.table_patch(entry.table, address, &data.values()));
        let mut patch = json!({"poll_errors": {entry.label(): Value::Null}});
        if let Some(name) = &entry.name {
            let value = match data {
                ReadData::Bits(bits) if entry.count == 1 => json!(bits.first().copied()),
                ReadData::Bits(bits) => json!(bits),
                ReadData::Registers(regs) => decode(regs, entry.format, self.order),
            };
            let key = match &value {
                Value::Array(_) => "list",
                Value::Bool(_) => "bool",
                Value::String(_) => "text",
                _ => "number",
            };
            patch["values"] = json!({ name.clone(): { key: value } });
        }
        cx.state(patch);
    }

    fn answered(&mut self, cx: &mut Cx, transaction: u16, pdu: Vec<u8>) {
        let Some((t, _, _)) = &self.in_flight else {
            return;
        };
        if *t != transaction {
            // A late answer to a request already timed out.
            return;
        }
        let (_, request, sent_at) = self.in_flight.take().expect("in flight");
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        let ours = request.unit == self.unit;
        let function = request.pdu[0];
        let outcome: Result<Parsed, CommandError> = match pdu.first() {
            Some(&f) if f == function | 0x80 => {
                let code = pdu.get(1).copied().unwrap_or(0);
                Err(CommandError::DeviceError {
                    code: Some(code.to_string()),
                    message: exception_text(code).into(),
                })
            }
            Some(&f) if f == function => {
                parse_response(&request.pdu, &pdu).ok_or_else(|| CommandError::DeviceError {
                    code: None,
                    message: "the response does not match the request".into(),
                })
            }
            _ => Err(CommandError::DeviceError {
                code: None,
                message: "the response is for another function".into(),
            }),
        };
        match (&request.purpose, outcome) {
            (Purpose::Poll { entry }, Ok(Parsed::Read(data))) => {
                let address = u16::from_be_bytes([request.pdu[1], request.pdu[2]]);
                self.apply_entry(cx, *entry, address, &data);
                self.stamps.answered(cx, "status");
            }
            (Purpose::Poll { entry }, Err(e)) => {
                if let Some(entry) = self.entries.get(*entry) {
                    let text = match e {
                        CommandError::DeviceError { message, .. } => message,
                        other => other.to_string(),
                    };
                    cx.state(json!({"poll_errors": {entry.label(): text}}));
                }
            }
            (Purpose::ReadBack(table), Ok(Parsed::Read(data))) => {
                let address = u16::from_be_bytes([request.pdu[1], request.pdu[2]]);
                cx.state(self.table_patch(*table, address, &data.values()));
            }
            (Purpose::Identification, Ok(Parsed::Identification(id))) => {
                self.identified(cx, id, None);
            }
            (Purpose::Command { id, result }, outcome) => {
                let id = *id;
                let result = *result;
                match outcome {
                    Ok(parsed) => {
                        if ours {
                            self.reflect(cx, &request.pdu, &parsed);
                        }
                        let value = match (result, parsed) {
                            (Answer::Raw, Parsed::Read(data)) => Ok(Value::Array(data.values())),
                            (Answer::Decoded(f), Parsed::Read(ReadData::Registers(regs))) => {
                                Ok(decode(&regs, f, self.order))
                            }
                            (Answer::Fifo, Parsed::Fifo(v)) => Ok(json!(v)),
                            (Answer::Record, Parsed::Record(v)) => Ok(json!(v)),
                            (Answer::Identification, Parsed::Identification(objects)) => {
                                self.identified(cx, objects, Some((id, request.unit)));
                                return self.pump(cx);
                            }
                            (Answer::Ack, _) => Err(()),
                            _ => Err(()),
                        };
                        match value {
                            Ok(value) => cx.complete(id, Ok(Outcome::Value { value })),
                            Err(()) => cx.complete(id, Ok(Outcome::Ack)),
                        }
                    }
                    Err(e) => cx.complete(id, Err(e)),
                }
            }
            _ => {}
        }
        self.pump(cx);
    }

    /// What a command's answer says of the device's own tables.
    fn reflect(&mut self, cx: &mut Cx, request: &[u8], parsed: &Parsed) {
        let address = u16::from_be_bytes([request[1], request[2]]);
        match (request[0], parsed) {
            (f, Parsed::Read(data)) if f <= READ_INPUT_REGISTERS => {
                let table = match f {
                    READ_COILS => Table::Coils,
                    READ_DISCRETE_INPUTS => Table::DiscreteInputs,
                    READ_HOLDING_REGISTERS => Table::HoldingRegisters,
                    _ => Table::InputRegisters,
                };
                cx.state(self.table_patch(table, address, &data.values()));
            }
            (WRITE_SINGLE_COIL, _) => {
                let on = request[3] == 0xFF;
                cx.state(self.table_patch(Table::Coils, address, &[json!(on)]));
                self.reread(Table::Coils, address, 1);
            }
            (WRITE_SINGLE_REGISTER, _) => {
                let v = u16::from_be_bytes([request[3], request[4]]);
                cx.state(self.table_patch(Table::HoldingRegisters, address, &[json!(v)]));
                self.reread(Table::HoldingRegisters, address, 1);
            }
            (WRITE_MULTIPLE_COILS, _) => {
                let count = u16::from_be_bytes([request[3], request[4]]);
                let bits = unpack_bits(&request[6..], usize::from(count));
                let values: Vec<Value> = bits.into_iter().map(Value::Bool).collect();
                cx.state(self.table_patch(Table::Coils, address, &values));
                self.reread(Table::Coils, address, count);
            }
            (WRITE_MULTIPLE_REGISTERS, _) => {
                let count = u16::from_be_bytes([request[3], request[4]]);
                let values: Vec<Value> =
                    words(&request[6..]).into_iter().map(|v| json!(v)).collect();
                cx.state(self.table_patch(Table::HoldingRegisters, address, &values));
                self.reread(Table::HoldingRegisters, address, count);
            }
            (MASK_WRITE_REGISTER, _) => {
                // The result depends on the register's content: read it back.
                if self.monitor {
                    self.queue.push_back(Request {
                        unit: self.unit,
                        pdu: read_pdu(Table::HoldingRegisters, address, 1),
                        purpose: Purpose::ReadBack(Table::HoldingRegisters),
                    });
                }
                self.reread(Table::HoldingRegisters, address, 1);
            }
            (READ_WRITE_MULTIPLE_REGISTERS, Parsed::Read(data)) => {
                let write_address = u16::from_be_bytes([request[5], request[6]]);
                let write_count = u16::from_be_bytes([request[7], request[8]]);
                let written: Vec<Value> = words(&request[10..])
                    .into_iter()
                    .map(|v| json!(v))
                    .collect();
                cx.state(self.table_patch(Table::HoldingRegisters, write_address, &written));
                // The read is done after the write (§6.17).
                cx.state(self.table_patch(Table::HoldingRegisters, address, &data.values()));
                self.reread(Table::HoldingRegisters, write_address, write_count);
            }
            _ => {}
        }
    }

    /// Identification objects arrived: gather them, ask for the rest while
    /// more follow, and put them in state.
    fn identified(&mut self, cx: &mut Cx, id: Identification, command: Option<(CommandId, u8)>) {
        for (object, value) in &id.objects {
            self.identification
                .insert(object.to_string(), Value::String(value.clone()));
        }
        if id.more {
            let pdu = vec![
                ENCAPSULATED_INTERFACE,
                READ_DEVICE_IDENTIFICATION,
                id.code,
                id.next,
            ];
            let purpose = match command {
                Some((cid, _)) => Purpose::Command {
                    id: cid,
                    result: Answer::Identification,
                },
                None => Purpose::Identification,
            };
            let unit = command.map_or(self.unit, |(_, u)| u);
            // Continue at once, ahead of the rest.
            self.queue.push_front(Request { unit, pdu, purpose });
            return;
        }
        let objects = std::mem::take(&mut self.identification);
        let unit = command.map_or(self.unit, |(_, u)| u);
        if unit == self.unit {
            let mut device = Map::new();
            let mut others = Map::new();
            for (k, v) in &objects {
                match k.parse::<u8>().ok().and_then(object_name) {
                    Some(name) => {
                        device.insert(name.into(), v.clone());
                    }
                    None => {
                        others.insert(k.clone(), v.clone());
                    }
                }
            }
            device.insert("conformity_level".into(), json!(id.conformity));
            if !others.is_empty() {
                device.insert("objects".into(), Value::Object(others));
            }
            cx.state(json!({ "device": device }));
        }
        if let Some((cid, _)) = command {
            let named: Map<String, Value> = objects
                .into_iter()
                .map(|(k, v)| {
                    let name = k
                        .parse::<u8>()
                        .ok()
                        .and_then(object_name)
                        .map_or(k, str::to_string);
                    (name, v)
                })
                .collect();
            cx.complete(
                cid,
                Ok(Outcome::Value {
                    value: json!({"conformity_level": id.conformity, "objects": named}),
                }),
            );
        }
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

/// The basic and regular object ids (§6.21).
fn object_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0 => "vendor_name",
        1 => "product_code",
        2 => "revision",
        3 => "vendor_url",
        4 => "product_name",
        5 => "model_name",
        6 => "user_application_name",
        _ => return None,
    })
}

fn read_pdu(table: Table, address: u16, count: u16) -> Vec<u8> {
    let mut pdu = vec![table.read_function()];
    pdu.extend_from_slice(&address.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    pdu
}

/// Read bits or registers.
#[derive(Debug, Clone, PartialEq)]
enum ReadData {
    Bits(Vec<bool>),
    Registers(Vec<u16>),
}

impl ReadData {
    fn values(&self) -> Vec<Value> {
        match self {
            ReadData::Bits(b) => b.iter().map(|&v| Value::Bool(v)).collect(),
            ReadData::Registers(r) => r.iter().map(|&v| json!(v)).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Identification {
    code: u8,
    conformity: u8,
    more: bool,
    next: u8,
    objects: Vec<(u8, String)>,
}

#[derive(Debug, Clone, PartialEq)]
enum Parsed {
    Read(ReadData),
    Written,
    Fifo(Vec<u16>),
    Record(Vec<u16>),
    Identification(Identification),
}

/// A normal response, checked against its request (§6).
fn parse_response(request: &[u8], pdu: &[u8]) -> Option<Parsed> {
    let function = request[0];
    match function {
        READ_COILS | READ_DISCRETE_INPUTS => {
            let count = usize::from(u16::from_be_bytes([request[3], request[4]]));
            let n = usize::from(*pdu.get(1)?);
            let data = pdu.get(2..2 + n)?;
            (n == count.div_ceil(8)).then(|| Parsed::Read(ReadData::Bits(unpack_bits(data, count))))
        }
        READ_HOLDING_REGISTERS | READ_INPUT_REGISTERS => {
            let count = usize::from(u16::from_be_bytes([request[3], request[4]]));
            let n = usize::from(*pdu.get(1)?);
            let data = pdu.get(2..2 + n)?;
            (n == count * 2).then(|| Parsed::Read(ReadData::Registers(words(data))))
        }
        READ_WRITE_MULTIPLE_REGISTERS => {
            let count = usize::from(u16::from_be_bytes([request[3], request[4]]));
            let n = usize::from(*pdu.get(1)?);
            let data = pdu.get(2..2 + n)?;
            (n == count * 2).then(|| Parsed::Read(ReadData::Registers(words(data))))
        }
        WRITE_SINGLE_COIL | WRITE_SINGLE_REGISTER | MASK_WRITE_REGISTER => {
            (pdu == request).then_some(Parsed::Written)
        }
        WRITE_MULTIPLE_COILS | WRITE_MULTIPLE_REGISTERS => {
            (pdu.get(..5)? == &request[..5]).then_some(Parsed::Written)
        }
        WRITE_FILE_RECORD => (pdu == request).then_some(Parsed::Written),
        READ_FIFO_QUEUE => {
            let n = usize::from(u16::from_be_bytes([*pdu.get(1)?, *pdu.get(2)?]));
            let count = usize::from(u16::from_be_bytes([*pdu.get(3)?, *pdu.get(4)?]));
            let data = pdu.get(5..3 + n)?;
            (data.len() == count * 2).then(|| Parsed::Fifo(words(data)))
        }
        READ_FILE_RECORD => {
            // One sub-request: response data length, then the sub-response
            // (its length, reference type 6, the record data).
            let sub_len = usize::from(*pdu.get(2)?);
            if *pdu.get(3)? != 6 || sub_len < 1 {
                return None;
            }
            let data = pdu.get(4..3 + sub_len)?;
            Some(Parsed::Record(words(data)))
        }
        ENCAPSULATED_INTERFACE => {
            if *pdu.get(1)? != READ_DEVICE_IDENTIFICATION {
                return None;
            }
            let code = *pdu.get(2)?;
            let conformity = *pdu.get(3)?;
            let more = *pdu.get(4)? == 0xFF;
            let next = *pdu.get(5)?;
            let count = usize::from(*pdu.get(6)?);
            let mut objects = Vec::new();
            let mut at = 7;
            for _ in 0..count {
                let id = *pdu.get(at)?;
                let len = usize::from(*pdu.get(at + 1)?);
                let value = pdu.get(at + 2..at + 2 + len)?;
                objects.push((id, String::from_utf8_lossy(value).into_owned()));
                at += 2 + len;
            }
            Some(Parsed::Identification(Identification {
                code,
                conformity,
                more,
                next,
                objects,
            }))
        }
        _ => None,
    }
}

/// A list of 0/1 or true/false values.
fn parse_bits(text: &str) -> Option<Vec<bool>> {
    text.split([',', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "1" | "true" | "on" => Some(true),
            "0" | "false" | "off" => Some(false),
            _ => None,
        })
        .collect()
}

/// A list of register values, 0 to 65535 or -32768 to -1.
fn parse_registers(text: &str) -> Option<Vec<u16>> {
    text.split([',', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let v: i64 = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()?
            } else {
                s.parse().ok()?
            };
            match v {
                0..=65535 => Some(v as u16),
                -32768..=-1 => Some(v as i16 as u16),
                _ => None,
            }
        })
        .collect()
}

fn u16_param(params: &Params, key: &str) -> Option<u16> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
}

impl Module for Modbus {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({"unit": self.unit}));
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "refresh" {
            if self.closed || !self.open {
                cx.complete(id, Err(CommandError::NotConnected));
                return;
            }
            self.sweep();
            self.queue.push_back(Request {
                unit: self.unit,
                pdu: vec![ENCAPSULATED_INTERFACE, READ_DEVICE_IDENTIFICATION, 1, 0],
                purpose: Purpose::Identification,
            });
            self.pump(cx);
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let unit = params
            .get("unit")
            .and_then(Value::as_u64)
            .map_or(self.unit, |u| u.min(255) as u8);
        let address = u16_param(params, "address").unwrap_or(0);
        let count = u16_param(params, "count").unwrap_or(1);
        let order = match params.get("word_order").and_then(Value::as_str) {
            Some("big") => WordOrder::Big,
            Some("little") => WordOrder::Little,
            _ => self.order,
        };
        let range_ok = |count: u16, max: u16| {
            count >= 1 && count <= max && u32::from(address) + u32::from(count) <= 65536
        };
        let (pdu, result) = match name {
            "read_coils"
            | "read_discrete_inputs"
            | "read_holding_registers"
            | "read_input_registers" => {
                let table = match name {
                    "read_coils" => Table::Coils,
                    "read_discrete_inputs" => Table::DiscreteInputs,
                    "read_holding_registers" => Table::HoldingRegisters,
                    _ => Table::InputRegisters,
                };
                if !range_ok(count, table.max_read()) {
                    return Modbus::fail(
                        cx,
                        id,
                        format!(
                            "count is 1 to {} and the range ends at address 65535",
                            table.max_read()
                        ),
                    );
                }
                (read_pdu(table, address, count), Answer::Raw)
            }
            "read_value" => {
                let table = match params.get("table").and_then(Value::as_str) {
                    Some("input") => Table::InputRegisters,
                    _ => Table::HoldingRegisters,
                };
                let format = params
                    .get("format")
                    .and_then(Value::as_str)
                    .and_then(Format::parse)
                    .unwrap_or(Format::U16);
                let count = match format.width() {
                    Some(w) => w,
                    None => count,
                };
                if !range_ok(count, MAX_READ_REGISTERS) {
                    return Modbus::fail(cx, id, "the value runs past address 65535");
                }
                (read_pdu(table, address, count), Answer::Decoded(format))
            }
            "write_coil" => {
                let on = params
                    .get("value")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let mut pdu = vec![WRITE_SINGLE_COIL];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(if on { &[0xFF, 0x00] } else { &[0x00, 0x00] });
                (pdu, Answer::Ack)
            }
            "write_register" => {
                let value = params.get("value").and_then(Value::as_i64).unwrap_or(0);
                let Some(v) = parse_registers(&value.to_string()).and_then(|v| v.first().copied())
                else {
                    return Modbus::fail(cx, id, "value is 0 to 65535, or -32768 to -1");
                };
                let mut pdu = vec![WRITE_SINGLE_REGISTER];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&v.to_be_bytes());
                (pdu, Answer::Ack)
            }
            "write_coils" => {
                let Some(bits) = params
                    .get("values")
                    .and_then(Value::as_str)
                    .and_then(parse_bits)
                else {
                    return Modbus::fail(cx, id, "values is a list of 0 and 1 (or true and false)");
                };
                let n = bits.len() as u16;
                if !range_ok(n, MAX_WRITE_BITS) {
                    return Modbus::fail(cx, id, "1 to 1968 coils, ending at address 65535");
                }
                let packed = pack_bits(&bits);
                let mut pdu = vec![WRITE_MULTIPLE_COILS];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&n.to_be_bytes());
                pdu.push(packed.len() as u8);
                pdu.extend(packed);
                (pdu, Answer::Ack)
            }
            "write_registers" | "write_value" => {
                let regs = if name == "write_registers" {
                    params
                        .get("values")
                        .and_then(Value::as_str)
                        .and_then(parse_registers)
                } else {
                    let format = params
                        .get("format")
                        .and_then(Value::as_str)
                        .and_then(Format::parse)
                        .unwrap_or(Format::U16);
                    params
                        .get("value")
                        .and_then(Value::as_f64)
                        .and_then(|v| encode(v, format, order))
                };
                let Some(regs) = regs else {
                    return Modbus::fail(
                        cx,
                        id,
                        if name == "write_registers" {
                            "values is a list of numbers, 0 to 65535 or -32768 to -1"
                        } else {
                            "the value does not fit the format"
                        },
                    );
                };
                let n = regs.len() as u16;
                if !range_ok(n, MAX_WRITE_REGISTERS) {
                    return Modbus::fail(cx, id, "1 to 123 registers, ending at address 65535");
                }
                let mut pdu = vec![WRITE_MULTIPLE_REGISTERS];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&n.to_be_bytes());
                pdu.push((n * 2) as u8);
                pdu.extend(word_bytes(&regs));
                (pdu, Answer::Ack)
            }
            "mask_write_register" => {
                let and = u16_param(params, "and_mask").unwrap_or(0xFFFF);
                let or = u16_param(params, "or_mask").unwrap_or(0);
                let mut pdu = vec![MASK_WRITE_REGISTER];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&and.to_be_bytes());
                pdu.extend_from_slice(&or.to_be_bytes());
                (pdu, Answer::Ack)
            }
            "read_write_registers" => {
                let write_address = u16_param(params, "write_address").unwrap_or(0);
                let Some(regs) = params
                    .get("values")
                    .and_then(Value::as_str)
                    .and_then(parse_registers)
                else {
                    return Modbus::fail(cx, id, "values is a list of numbers, 0 to 65535");
                };
                let n = regs.len() as u16;
                if !range_ok(count, MAX_READ_REGISTERS)
                    || n == 0
                    || n > MAX_RW_WRITE_REGISTERS
                    || u32::from(write_address) + u32::from(n) > 65536
                {
                    return Modbus::fail(
                        cx,
                        id,
                        "read 1 to 125 and write 1 to 121 registers, each ending at address 65535",
                    );
                }
                let mut pdu = vec![READ_WRITE_MULTIPLE_REGISTERS];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&count.to_be_bytes());
                pdu.extend_from_slice(&write_address.to_be_bytes());
                pdu.extend_from_slice(&n.to_be_bytes());
                pdu.push((n * 2) as u8);
                pdu.extend(word_bytes(&regs));
                (pdu, Answer::Raw)
            }
            "read_fifo_queue" => {
                let mut pdu = vec![READ_FIFO_QUEUE];
                pdu.extend_from_slice(&address.to_be_bytes());
                (pdu, Answer::Fifo)
            }
            "read_file_record" | "write_file_record" => {
                let file = u16_param(params, "file").unwrap_or(1);
                let record = u16_param(params, "record").unwrap_or(0);
                if file == 0 || record > MAX_RECORD {
                    return Modbus::fail(cx, id, "file is 1 to 65535 and record 0 to 9999");
                }
                if name == "read_file_record" {
                    let length = u16_param(params, "length").unwrap_or(1);
                    // One sub-response must fit a response of 253 bytes.
                    if length == 0 || length > 124 {
                        return Modbus::fail(cx, id, "length is 1 to 124 registers");
                    }
                    let mut pdu = vec![READ_FILE_RECORD, 7, 6];
                    pdu.extend_from_slice(&file.to_be_bytes());
                    pdu.extend_from_slice(&record.to_be_bytes());
                    pdu.extend_from_slice(&length.to_be_bytes());
                    (pdu, Answer::Record)
                } else {
                    let Some(regs) = params
                        .get("values")
                        .and_then(Value::as_str)
                        .and_then(parse_registers)
                    else {
                        return Modbus::fail(cx, id, "values is a list of numbers, 0 to 65535");
                    };
                    if regs.is_empty() || regs.len() > 122 {
                        return Modbus::fail(cx, id, "1 to 122 registers");
                    }
                    let n = regs.len() as u16;
                    let mut pdu = vec![WRITE_FILE_RECORD, (7 + n * 2) as u8, 6];
                    pdu.extend_from_slice(&file.to_be_bytes());
                    pdu.extend_from_slice(&record.to_be_bytes());
                    pdu.extend_from_slice(&n.to_be_bytes());
                    pdu.extend(word_bytes(&regs));
                    (pdu, Answer::Ack)
                }
            }
            "read_device_identification" => {
                let code = match params.get("category").and_then(Value::as_str) {
                    Some("regular") => 2,
                    Some("extended") => 3,
                    _ => 1,
                };
                self.identification.clear();
                (
                    vec![ENCAPSULATED_INTERFACE, READ_DEVICE_IDENTIFICATION, code, 0],
                    Answer::Identification,
                )
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
        self.enqueue(
            cx,
            Request {
                unit,
                pdu,
                purpose: Purpose::Command { id, result },
            },
        );
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                self.heard_at = cx.now();
                if self.monitor {
                    self.queue.push_back(Request {
                        unit: self.unit,
                        pdu: vec![ENCAPSULATED_INTERFACE, READ_DEVICE_IDENTIFICATION, 1, 0],
                        purpose: Purpose::Identification,
                    });
                    self.sweep();
                }
                if !self.monitor || self.entries.is_empty() {
                    self.queue.push_back(self.liveness());
                }
                cx.set_timer(POLL_TIMER, self.poll_every);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for (transaction, _unit, pdu) in self.deframer.feed(&bytes) {
                    self.answered(cx, transaction, pdu);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.connect(cx),
            REPLY_TIMER => {
                if let Some((_, request, _)) = self.in_flight.take() {
                    if let Purpose::Command { id, .. } = request.purpose {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES {
                        cx.log(
                            Level::Warning,
                            "the Modbus server answered none of three requests",
                        );
                        cx.tcp_close(SOCKET);
                        self.lost(cx, "no response from the Modbus server".into());
                        return;
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.open => {
                if self.monitor && !self.entries.is_empty() {
                    self.sweep();
                } else if self.in_flight.is_none()
                    && self.queue.is_empty()
                    && cx.now().saturating_sub(self.heard_at) >= self.poll_every
                {
                    self.queue.push_back(self.liveness());
                }
                self.pump(cx);
                cx.set_timer(POLL_TIMER, self.poll_every);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

impl Modbus {
    fn liveness(&self) -> Request {
        Request {
            unit: self.unit,
            pdu: read_pdu(Table::HoldingRegisters, self.liveness_address, 1),
            purpose: Purpose::Liveness,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> Modbus {
        let spec = Catalog::source_tree().devices["generic-modbus"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Modbus::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn opened(m: &mut Modbus) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    /// The response to the request in flight, with its transaction id.
    fn answer(m: &mut Modbus, now: Millis, pdu: &[u8]) -> Vec<Action> {
        let (t, request, _) = m.in_flight.clone().expect("a request in flight");
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(adu(t, request.unit, pdu)));
        cx.take()
    }

    fn run(m: &mut Modbus, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn requests_carry_the_mbap_header() {
        // Read 2 holding registers from address 107 of unit 17: the
        // Application Protocol's example PDU 03 006B 0002 in an MBAP header.
        assert_eq!(
            adu(1, 17, &[3, 0x00, 0x6B, 0x00, 0x02]),
            [0, 1, 0, 0, 0, 6, 17, 3, 0x00, 0x6B, 0x00, 0x02]
        );
        let mut d = Deframer::default();
        let frame = adu(9, 1, &[3, 4, 0, 1, 0, 2]);
        assert!(d.feed(&frame[..5]).is_empty());
        assert_eq!(d.feed(&frame[5..]), [(9, 1, vec![3, 4, 0, 1, 0, 2])]);
    }

    #[test]
    fn values_decode_and_encode_in_either_word_order() {
        // 123.456 as an IEEE 754 single is 42F6E979h.
        let regs = [0x42F6, 0xE979];
        let v = decode(&regs, Format::F32, WordOrder::Big);
        assert!((v.as_f64().unwrap() - 123.456).abs() < 1e-4);
        assert_eq!(
            encode(123.456, Format::F32, WordOrder::Little),
            Some(vec![0xE979, 0x42F6])
        );
        assert_eq!(decode(&[0xFFFF], Format::I16, WordOrder::Big), json!(-1));
        assert_eq!(
            decode(&[0x0001, 0x0000], Format::U32, WordOrder::Little),
            json!(1)
        );
        assert_eq!(
            decode(&[0x4142, 0x4300], Format::String, WordOrder::Big),
            json!("ABC")
        );
        assert_eq!(
            decode(&[1, 2, 3], Format::U16, WordOrder::Big),
            json!([1, 2, 3])
        );
        assert_eq!(encode(70000.0, Format::U16, WordOrder::Big), None);
        assert_eq!(encode(1.5, Format::I32, WordOrder::Big), None);
        assert_eq!(
            encode(-2.0, Format::I32, WordOrder::Big),
            Some(vec![0xFFFF, 0xFFFE])
        );
        assert_eq!(
            decode(&[0x7FC0, 0], Format::F32, WordOrder::Big),
            json!("nan")
        );
    }

    #[test]
    fn the_poll_list_is_parsed_and_checked() {
        let entries = parse_poll("temp=ir:30:f32, co:0:16 hr:100;label=hr:200:4:string").unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].format, Format::F32);
        assert_eq!(entries[1].table, Table::Coils);
        assert_eq!(entries[1].count, 16);
        assert_eq!(entries[2].label(), "hr:100");
        assert_eq!(entries[3].format, Format::String);
        assert!(parse_poll("hr:0:3:f32").is_err());
        assert!(parse_poll("co:0:u16").is_err());
        assert!(parse_poll("x:0").is_err());
        assert!(parse_poll("hr:65535:2").is_err());
        assert!(parse_poll("big=hr:0:200").is_err());
        // An unnamed range longer than a request is read in several.
        assert!(parse_poll("hr:0:300").is_ok());
    }

    #[test]
    fn monitored_it_reads_the_identification_and_the_poll_list() {
        let mut m = module(true, json!({"poll": "temp=ir:30:f32, co:0:10, hr:0:300"}));
        let a = opened(&mut m);
        // Identification first.
        assert_eq!(sent(&a), [adu(1, 1, &[0x2B, 0x0E, 1, 0])]);
        let a = answer(
            &mut m,
            3,
            &[
                0x2B, 0x0E, 1, 0x81, 0, 0, 2, 0, 4, b'A', b'c', b'm', b'e', 1, 2, b'P', b'1',
            ],
        );
        assert!(a.contains(&Action::RoundTrip(3)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(
            state(&a)["device"],
            json!({"vendor_name": "Acme", "product_code": "P1", "conformity_level": 0x81})
        );
        assert_eq!(sent(&a), [adu(2, 1, &[4, 0, 30, 0, 2])]);
        let a = answer(&mut m, 4, &[4, 4, 0x42, 0xF6, 0xE9, 0x79]);
        let s = state(&a);
        assert_eq!(s["input_registers"], json!({"30": 0x42F6, "31": 0xE979}));
        assert!((s["values"]["temp"]["number"].as_f64().unwrap() - 123.456).abs() < 1e-4);
        assert_eq!(sent(&a), [adu(3, 1, &[1, 0, 0, 0, 10])]);
        let a = answer(&mut m, 5, &[1, 2, 0b0000_0101, 0b10]);
        let coils = &state(&a)["coils"];
        assert_eq!(coils["0"], json!(true));
        assert_eq!(coils["1"], json!(false));
        assert_eq!(coils["2"], json!(true));
        assert_eq!(coils["9"], json!(true));
        // 300 registers take three requests of at most 125.
        assert_eq!(sent(&a), [adu(4, 1, &[3, 0, 0, 0, 125])]);
        let mut data = vec![3, 250];
        data.extend(vec![0; 250]);
        let a = answer(&mut m, 6, &data);
        assert_eq!(sent(&a), [adu(5, 1, &[3, 0, 125, 0, 125])]);
    }

    #[test]
    fn writes_are_confirmed_by_their_echo_and_read_back() {
        let mut m = module(true, json!({"poll": "setpoint=hr:10:f32"}));
        opened(&mut m);
        answer(&mut m, 1, &[0xAB, 1]); // identification: illegal function
        answer(&mut m, 2, &[3, 4, 0, 0, 0, 0]);

        let a = run(
            &mut m,
            1,
            "write_value",
            json!({"address": 10, "value": 1.5, "format": "f32"}),
        );
        // 1.5 is 3FC00000h.
        assert_eq!(
            sent(&a),
            [adu(3, 1, &[16, 0, 10, 0, 2, 4, 0x3F, 0xC0, 0, 0])]
        );
        let a = answer(&mut m, 3, &[16, 0, 10, 0, 2]);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(
            state(&a)["holding_registers"],
            json!({"10": 0x3FC0, "11": 0})
        );
        // The polled entry it touched is read again.
        assert_eq!(sent(&a), [adu(4, 1, &[3, 0, 10, 0, 2])]);
        answer(&mut m, 4, &[3, 4, 0x3F, 0xC0, 0, 0]);

        let a = run(
            &mut m,
            2,
            "write_coil",
            json!({"address": 3, "value": true}),
        );
        assert_eq!(sent(&a), [adu(5, 1, &[5, 0, 3, 0xFF, 0])]);
        let a = answer(&mut m, 5, &[5, 0, 3, 0xFF, 0]);
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert_eq!(state(&a)["coils"], json!({"3": true}));

        let a = run(
            &mut m,
            3,
            "write_coils",
            json!({"address": 19, "values": "1,0,1,1,0,0,1,1,1,0"}),
        );
        // The Application Protocol's example: 10 coils from 20 (address 19), CD 01.
        assert_eq!(
            sent(&a),
            [adu(6, 1, &[15, 0, 0x13, 0, 0x0A, 2, 0xCD, 0x01])]
        );
        answer(&mut m, 6, &[15, 0, 0x13, 0, 0x0A]);

        let a = run(
            &mut m,
            4,
            "write_register",
            json!({"address": 1, "value": -2}),
        );
        assert_eq!(sent(&a), [adu(7, 1, &[6, 0, 1, 0xFF, 0xFE])]);
        // An exception fails it with its code.
        let a = answer(&mut m, 7, &[0x86, 2]);
        assert_eq!(
            completed(&a, 4),
            Some(Err(CommandError::DeviceError {
                code: Some("2".into()),
                message: "illegal data address".into()
            }))
        );

        let a = run(
            &mut m,
            5,
            "mask_write_register",
            json!({"address": 4, "and_mask": 0xF2, "or_mask": 0x25}),
        );
        assert_eq!(sent(&a), [adu(8, 1, &[22, 0, 4, 0, 0xF2, 0, 0x25])]);
        let a = answer(&mut m, 8, &[22, 0, 4, 0, 0xF2, 0, 0x25]);
        assert_eq!(completed(&a, 5), Some(Ok(Outcome::Ack)));
        // The register is read back.
        assert_eq!(sent(&a), [adu(9, 1, &[3, 0, 4, 0, 1])]);
        let a = answer(&mut m, 9, &[3, 2, 0, 0x17]);
        assert_eq!(state(&a)["holding_registers"], json!({"4": 0x17}));
    }

    #[test]
    fn reads_return_lists_and_other_units_leave_state_alone() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        answer(&mut m, 1, &[3, 2, 0, 0]);
        let a = run(
            &mut m,
            1,
            "read_input_registers",
            json!({"address": 8, "count": 1, "unit": 5}),
        );
        assert_eq!(sent(&a), [adu(2, 5, &[4, 0, 8, 0, 1])]);
        let a = answer(&mut m, 1, &[4, 2, 0, 10]);
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!([10]) }))
        );
        assert_eq!(state(&a), json!({}));

        run(&mut m, 2, "read_fifo_queue", json!({"address": 0x04DE}));
        let a = answer(&mut m, 1, &[24, 0, 6, 0, 2, 0x01, 0xB8, 0x12, 0x84]);
        assert_eq!(
            completed(&a, 2),
            Some(Ok(Outcome::Value {
                value: json!([0x01B8, 0x1284])
            }))
        );

        let a = run(
            &mut m,
            3,
            "read_file_record",
            json!({"file": 4, "record": 1, "length": 2}),
        );
        assert_eq!(sent(&a), [adu(4, 1, &[20, 7, 6, 0, 4, 0, 1, 0, 2])]);
        let a = answer(&mut m, 1, &[20, 6, 5, 6, 0x0D, 0xFE, 0x00, 0x20]);
        assert_eq!(
            completed(&a, 3),
            Some(Ok(Outcome::Value {
                value: json!([0x0DFE, 0x0020])
            }))
        );

        let a = run(
            &mut m,
            4,
            "read_write_registers",
            json!({"address": 3, "count": 1, "write_address": 14, "values": "255"}),
        );
        assert_eq!(
            sent(&a),
            [adu(5, 1, &[23, 0, 3, 0, 1, 0, 14, 0, 1, 2, 0, 0xFF])]
        );
        let a = answer(&mut m, 1, &[23, 2, 0, 0xFE]);
        assert_eq!(
            completed(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!([0xFE])
            }))
        );
        assert_eq!(
            state(&a)["holding_registers"],
            json!({"3": 0xFE, "14": 0xFF})
        );
    }

    #[test]
    fn commands_only_reads_one_liveness_register_and_nothing_else() {
        let mut m = module(false, json!({"poll": "hr:0:4", "liveness_address": 9}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [adu(1, 1, &[3, 0, 9, 0, 1])]);
        // An exception still shows the server is there.
        let a = answer(&mut m, 2, &[0x83, 2]);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(2)));
        assert_eq!(state(&a), json!({}));
        // Heard within the interval: no liveness read.
        let mut cx = Cx::new(500);
        m.timer(&mut cx, POLL_TIMER);
        assert!(sent(&cx.take()).is_empty());
        let mut cx = Cx::new(2_500);
        m.timer(&mut cx, POLL_TIMER);
        assert_eq!(sent(&cx.take()), [adu(2, 1, &[3, 0, 9, 0, 1])]);
        // A command waits for the request in flight, then goes.
        let a = run(&mut m, 1, "read_coils", json!({"address": 0}));
        assert!(sent(&a).is_empty());
        let a = answer(&mut m, 2_600, &[3, 2, 0, 0]);
        assert_eq!(sent(&a), [adu(3, 1, &[1, 0, 0, 0, 1])]);
    }

    #[test]
    fn three_timeouts_drop_the_connection() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        let mut last = Vec::new();
        for id in 1..=3 {
            if m.in_flight.is_none() {
                run(&mut m, id, "read_coils", json!({"address": 0}));
            }
            let mut cx = Cx::new(0);
            m.timer(&mut cx, REPLY_TIMER);
            last = cx.take();
        }
        assert!(last.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(last
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(last.contains(&Action::SetTimer {
            key: RETRY_TIMER,
            after: RETRY_MIN
        }));
    }
}
