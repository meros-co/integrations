//! The recovery journal an RTP-MIDI sender puts in every packet, so a peer
//! that loses packets can repair what they carried (RFC 6295, "RTP Payload
//! Format for MIDI", §4-5, Appendices A and B).
//!
//! The journal codes the MIDI the core sent from the checkpoint packet up to
//! the packet before the one that carries it (the checkpoint history). The
//! checkpoint follows the closed-loop sending policy (Appendix C.2.2.2), the
//! default: it is the first packet of the session until the peer's receiver
//! feedback (`RS`, Apple's MIDI Network Driver Protocol) names the last
//! packet it has, and the packet after that one from then on.
//!
//! Every chapter is kept, each coding the most recent active command for
//! what it protects, as the appendices define "active" (not before a Reset
//! State command), "N-active" (not before all notes off, all sound off or a
//! mode change) and "C-active" (not before reset all controllers):
//!
//! - channel journals (A.2-A.9): P (program, with the bank select before
//!   it), C (controllers: the value tool, and the count tool for all sound
//!   off, reset all controllers, all notes off, omni off and on and poly),
//!   M (RPN and NRPN transactions, with the value and count tools), W (pitch
//!   wheel), N (notes on and off), E (repeated note ons and release
//!   velocities), T (channel pressure) and A (poly pressure);
//! - the system journal (B.1-B.5): D (reset, tune request, song select and
//!   the undefined system commands), V (active sensing), Q (the sequencer:
//!   start, stop, continue, clock and song position), F (MIDI Time Code:
//!   the last complete time and a partial one) and X (SysEx, with the
//!   recency tool, SysEx of identical data being of one type).
//!
//! S bits are cleared for what the packet before carried (Appendix A.1).
//! The H bits are 0: no controller uses the enhanced Chapter C encoding.
//!
//! A protocol limit: the system journal's LENGTH has 10 bits, so it holds at
//! most 1023 bytes. SysEx logs that would not fit are left out, oldest
//! first, so a SysEx of more than about 1000 data bytes (or many SysEx sent
//! within one feedback interval) is not fully protected.

use std::collections::{BTreeMap, HashMap};

use crate::midi::{Message, Rate, Timecode};

/// When and in which order a command went out: the extended sequence
/// number of its packet, and its place among every command sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Stamp {
    seq: u64,
    order: u64,
}

/// What a packet carried: whole messages, or a segment of a SysEx too long
/// for one packet.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    Message(Message),
    Segment {
        data: Vec<u8>,
        first: bool,
        last: bool,
    },
}

/// Controllers coded with the count tool: their value means nothing (all
/// sound off, reset all controllers, all notes off) or is fixed (omni off
/// and on, poly).
fn count_tool(controller: u8) -> bool {
    matches!(controller, 120 | 121 | 123 | 124 | 125 | 127)
}

const LENGTH_MAX: usize = 1023;

#[derive(Debug, Clone, Copy)]
struct Controller {
    value: u8,
    /// Commands for the number since the session or a Reset State began,
    /// modulo 64 (the count tool).
    count: u8,
    stamp: Stamp,
}

#[derive(Debug, Clone, Copy)]
struct Note {
    on: bool,
    /// The note on's velocity, or the note off's release velocity.
    velocity: u8,
    stamp: Stamp,
    /// When the note on went out, for the Y bit.
    at: u64,
}

#[derive(Debug, Clone, Default)]
struct Param {
    entry_msb: Option<(u8, u64)>,
    entry_lsb: Option<(u8, u64)>,
    /// Data increments less decrements since the last data entry
    /// (A-BUTTON), and those not before reset all controllers (C-BUTTON).
    a_button: i32,
    c_button: i32,
    /// The order of the last increment or decrement counted.
    button_order: Option<u64>,
    /// Initiated transactions, modulo 128.
    count: u8,
    count_order: u64,
    stamp: Stamp,
}

/// A program change: the program, the bank select before it (MSB, LSB and
/// whether reset all controllers came between), and when.
type Program = (u8, Option<(u8, u8, bool)>, Stamp);

#[derive(Debug, Clone)]
struct Channel {
    program: Option<Program>,
    bank_msb: Option<u8>,
    bank_lsb: Option<u8>,
    reset_since_bank: bool,
    controllers: Vec<Option<Controller>>,
    counts: [u8; 128],
    /// The last parameter number MSB (99 or 101) not closed by reset all
    /// controllers: NRPN, value.
    msb: Option<(bool, u8)>,
    /// The most recent C-active transaction command is an MSB.
    pending: bool,
    /// The parameter of the transaction in progress.
    selected: Option<(bool, u16)>,
    /// The most recent C-active transaction command.
    transaction: Option<Stamp>,
    params: BTreeMap<(bool, u16), Param>,
    reset_controllers: Option<u64>,
    wheel: Option<([u8; 2], Stamp)>,
    notes: [Option<Note>; 128],
    /// Note ons less note offs per note (Chapter E's reference count).
    references: [u32; 128],
    last_note_off: Option<u64>,
    pressure: Option<(u8, Stamp)>,
    poly: [Option<(u8, Stamp, bool)>; 128],
}

impl Default for Channel {
    fn default() -> Channel {
        Channel {
            program: None,
            bank_msb: None,
            bank_lsb: None,
            reset_since_bank: false,
            controllers: vec![None; 128],
            counts: [0; 128],
            msb: None,
            pending: false,
            selected: None,
            transaction: None,
            params: BTreeMap::new(),
            reset_controllers: None,
            wheel: None,
            notes: [None; 128],
            references: [0; 128],
            last_note_off: None,
            pressure: None,
            poly: [None; 128],
        }
    }
}

/// MTC as Chapter F codes it.
#[derive(Debug, Clone, Default)]
struct Mtc {
    /// The last complete time: quarter frame nibbles (already two frames on
    /// when they ran forward) or a full message's hr mn sc fr.
    complete: Option<([u8; 8], bool)>,
    full: Option<[u8; 4]>,
    nibbles: [u8; 8],
    /// The run of quarter frames in progress: its direction and the last
    /// piece, when it is a partial frame value.
    partial: Option<(bool, u8)>,
    last_piece: Option<u8>,
    reverse: bool,
    stamp: Option<Stamp>,
}

#[derive(Debug, Clone, Default)]
struct Sequencer {
    running: bool,
    /// The song position in MIDI clocks.
    position: u32,
    downbeat: bool,
    /// Start came after the last continue.
    started: bool,
    stamp: Option<Stamp>,
}

#[derive(Debug, Clone)]
struct SysEx {
    data: Vec<u8>,
    finished: bool,
    tcount: u8,
    stamp: Stamp,
}

/// The sender's journal of one session.
#[derive(Debug, Default)]
pub(crate) struct Journal {
    channels: Vec<Channel>,
    order: u64,
    /// The extended sequence number of the session's first packet, and of
    /// the checkpoint packet.
    first: Option<u64>,
    checkpoint: u64,
    last_sent: Option<u64>,
    reset_count: u8,
    reset_stamp: Option<Stamp>,
    tune: Option<(u8, Stamp)>,
    tune_count: u8,
    song: Option<(u8, Stamp)>,
    /// F4, F5, F9 and FD: count (modulo 256) and the last active one.
    undefined: [(u8, Option<Stamp>); 4],
    sense_count: u8,
    sense: Option<Stamp>,
    sequencer: Sequencer,
    mtc: Mtc,
    sysex: Vec<SysEx>,
    unfinished: Option<SysEx>,
    types: HashMap<Vec<u8>, u8>,
}

fn s_bit(stamp: Stamp, previous: Option<u64>) -> u8 {
    if Some(stamp.seq) == previous {
        0
    } else {
        0x80
    }
}

impl Journal {
    pub(crate) fn new() -> Journal {
        Journal {
            channels: vec![Channel::default(); 16],
            ..Journal::default()
        }
    }

    /// The peer has every packet up to `seq` (16 bits, as `RS` gives it):
    /// the checkpoint moves to the packet after it.
    pub(crate) fn acknowledge(&mut self, seq: u16) {
        let Some(last) = self.last_sent else { return };
        let behind = (last as u16).wrapping_sub(seq) as u64;
        if behind >= 0x8000 || behind > last {
            return;
        }
        let acked = last - behind;
        if self.first.is_some_and(|f| acked + 1 < f) {
            return;
        }
        self.checkpoint = self.checkpoint.max(acked + 1);
    }

    /// Whether the peer has acknowledged packet `seq` (extended).
    pub(crate) fn acknowledged(&self, seq: u64) -> bool {
        self.first.is_some() && self.checkpoint > seq
    }

    /// Take in what packet `seq` (extended) carried, sent at `now` (ms).
    pub(crate) fn record(&mut self, seq: u64, now: u64, items: &[Item]) {
        if self.first.is_none() {
            self.first = Some(seq);
            self.checkpoint = seq;
        }
        self.last_sent = Some(seq);
        for item in items {
            self.order += 1;
            let stamp = Stamp {
                seq,
                order: self.order,
            };
            match item {
                Item::Message(m) => self.message(m, stamp, now),
                Item::Segment { data, first, last } => self.segment(data, *first, *last, stamp),
            }
        }
    }

    fn reset_state(&mut self) {
        self.channels = vec![Channel::default(); 16];
        self.tune = None;
        self.song = None;
        for u in &mut self.undefined {
            u.1 = None;
        }
        self.sense = None;
        self.sequencer = Sequencer::default();
        self.mtc = Mtc::default();
        self.sysex.clear();
        self.unfinished = None;
    }

    fn message(&mut self, m: &Message, stamp: Stamp, now: u64) {
        match *m {
            Message::NoteOn {
                channel,
                note,
                velocity,
            } if velocity > 0 => {
                let c = &mut self.channels[channel as usize & 15];
                c.notes[note as usize & 127] = Some(Note {
                    on: true,
                    velocity,
                    stamp,
                    at: now,
                });
                c.references[note as usize & 127] += 1;
            }
            Message::NoteOn { channel, note, .. } => {
                self.note_off(channel, note, 64, stamp);
            }
            Message::NoteOff {
                channel,
                note,
                velocity,
            } => self.note_off(channel, note, velocity, stamp),
            Message::PolyPressure {
                channel,
                note,
                pressure,
            } => {
                self.channels[channel as usize & 15].poly[note as usize & 127] =
                    Some((pressure, stamp, false));
            }
            Message::ControlChange {
                channel,
                controller,
                value,
            } => self.control_change(channel, controller & 127, value, stamp),
            Message::ProgramChange { channel, program } => {
                let c = &mut self.channels[channel as usize & 15];
                let bank = c
                    .bank_msb
                    .map(|msb| (msb, c.bank_lsb.unwrap_or(0), c.reset_since_bank));
                c.program = Some((program, bank, stamp));
            }
            Message::ChannelPressure { channel, pressure } => {
                self.channels[channel as usize & 15].pressure = Some((pressure, stamp));
            }
            Message::PitchBend { channel, value } => {
                let bytes = [(value & 0x7F) as u8, (value >> 7 & 0x7F) as u8];
                self.channels[channel as usize & 15].wheel = Some((bytes, stamp));
            }
            Message::SysEx(ref body) => self.finished_sysex(body, stamp),
            Message::QuarterFrame { piece, value } => self.quarter_frame(piece & 7, value, stamp),
            Message::SongPosition(beats) => {
                let q = &mut self.sequencer;
                q.position = beats as u32 * 6;
                q.downbeat = false;
                q.stamp = Some(stamp);
            }
            Message::SongSelect(song) => self.song = Some((song, stamp)),
            Message::TuneRequest => {
                self.tune_count = (self.tune_count + 1) & 0x7F;
                self.tune = Some((self.tune_count, stamp));
            }
            Message::Undefined(status) | Message::RealTime(status) => {
                let i = match status {
                    0xF4 => 0,
                    0xF5 => 1,
                    0xF9 => 2,
                    _ => 3,
                };
                let u = &mut self.undefined[i];
                u.0 = u.0.wrapping_add(1);
                u.1 = Some(stamp);
            }
            Message::Clock => {
                let q = &mut self.sequencer;
                if q.running {
                    if q.downbeat {
                        q.position = (q.position + 1) % 524_288;
                    }
                    q.downbeat = true;
                    q.stamp = Some(stamp);
                }
            }
            Message::Start => {
                self.sequencer = Sequencer {
                    running: true,
                    position: 0,
                    downbeat: false,
                    started: true,
                    stamp: Some(stamp),
                };
            }
            Message::Continue => {
                let q = &mut self.sequencer;
                q.running = true;
                q.started = false;
                q.stamp = Some(stamp);
            }
            Message::Stop => {
                let q = &mut self.sequencer;
                q.running = false;
                q.stamp = Some(stamp);
            }
            Message::ActiveSensing => {
                self.sense_count = (self.sense_count + 1) & 0x7F;
                self.sense = Some(stamp);
            }
            Message::Reset => {
                self.reset_state();
                self.reset_count = (self.reset_count + 1) & 0x7F;
                self.reset_stamp = Some(stamp);
            }
        }
    }

    fn note_off(&mut self, channel: u8, note: u8, velocity: u8, stamp: Stamp) {
        let c = &mut self.channels[channel as usize & 15];
        let n = note as usize & 127;
        c.notes[n] = Some(Note {
            on: false,
            velocity,
            stamp,
            at: 0,
        });
        c.references[n] = c.references[n].saturating_sub(1);
        c.last_note_off = Some(stamp.seq);
    }

    fn control_change(&mut self, channel: u8, n: u8, value: u8, stamp: Stamp) {
        let order = stamp.order;
        let c = &mut self.channels[channel as usize & 15];
        c.counts[n as usize] = (c.counts[n as usize] + 1) & 0x3F;
        let in_transaction = c.selected.is_some() || c.pending;
        match n {
            98..=101 => {
                let nrpn = n == 98 || n == 99;
                c.transaction = Some(stamp);
                if n == 99 || n == 101 {
                    c.msb = Some((nrpn, value));
                    c.pending = true;
                    c.selected = None;
                } else {
                    let msb = match c.msb {
                        Some((q, m)) if q == nrpn => m,
                        _ => 0,
                    };
                    c.pending = false;
                    if msb == 127 && value == 127 {
                        c.selected = None;
                    } else {
                        let key = (nrpn, (msb as u16) << 7 | value as u16);
                        c.selected = Some(key);
                        let p = c.params.entry(key).or_default();
                        p.count = (p.count + 1) & 0x7F;
                        p.count_order = order;
                        p.stamp = stamp;
                    }
                }
            }
            6 | 38 | 96 | 97 if in_transaction => {
                if c.pending {
                    // An MSB with no LSB: the LSB is taken as 0.
                    let (nrpn, msb) = c.msb.unwrap_or((false, 0));
                    let key = (nrpn, (msb as u16) << 7);
                    c.selected = Some(key);
                    c.pending = false;
                    let p = c.params.entry(key).or_default();
                    p.count = (p.count + 1) & 0x7F;
                    p.count_order = order;
                }
                c.transaction = Some(stamp);
                let key = c.selected.expect("a transaction is in progress");
                let p = c.params.entry(key).or_default();
                p.stamp = stamp;
                match n {
                    6 => {
                        p.entry_msb = Some((value, order));
                        p.entry_lsb = None;
                        p.a_button = 0;
                        p.c_button = 0;
                        p.button_order = None;
                    }
                    38 => {
                        p.entry_lsb = Some((value, order));
                        p.a_button = 0;
                        p.c_button = 0;
                        p.button_order = None;
                    }
                    _ => {
                        let step = if n == 96 { 1 } else { -1 };
                        p.a_button = (p.a_button + step).clamp(-16383, 16383);
                        p.c_button = (p.c_button + step).clamp(-16383, 16383);
                        p.button_order = Some(order);
                    }
                }
            }
            _ => {
                c.controllers[n as usize] = Some(Controller {
                    value,
                    count: c.counts[n as usize],
                    stamp,
                });
                // Of a mutually exclusive pair only the most recent is coded.
                let other = match n {
                    124 => Some(125),
                    125 => Some(124),
                    126 => Some(127),
                    127 => Some(126),
                    _ => None,
                };
                if let Some(other) = other {
                    c.controllers[other] = None;
                }
            }
        }
        match n {
            0 => {
                c.bank_msb = Some(value);
                c.bank_lsb = None;
                c.reset_since_bank = false;
            }
            32 if c.bank_msb.is_some() => c.bank_lsb = Some(value),
            121 => {
                c.reset_controllers = Some(order);
                c.reset_since_bank |= c.bank_msb.is_some();
                c.wheel = None;
                c.pressure = None;
                c.poly = [None; 128];
                c.msb = None;
                c.pending = false;
                c.selected = None;
                c.transaction = None;
                for p in c.params.values_mut() {
                    p.c_button = 0;
                }
            }
            120 | 123..=127 => {
                c.notes = [None; 128];
                c.references = [0; 128];
                c.pressure = None;
                for p in c.poly.iter_mut().flatten() {
                    p.2 = true;
                }
            }
            _ => {}
        }
    }

    fn sysex_type(&mut self, data: &[u8]) -> u8 {
        let count = self.types.entry(data.to_vec()).or_insert(0);
        *count = count.wrapping_add(1);
        *count
    }

    fn finished_sysex(&mut self, body: &[u8], stamp: Stamp) {
        // Reset State SysEx: General MIDI 1 and 2 on and off, DLS on and off.
        let reset = matches!(
            body,
            [0x7E, _, 0x09, 0x00..=0x03] | [0x7E, _, 0x0A, 0x01 | 0x02]
        );
        if reset {
            self.reset_state();
        }
        // An MTC full message goes in Chapter F.
        if let [0x7F, _, 0x01, 0x01, hr, mn, sc, fr] = *body {
            let m = &mut self.mtc;
            m.full = Some([hr, mn, sc, fr]);
            m.complete = None;
            m.partial = None;
            m.last_piece = None;
            m.reverse = false;
            m.stamp = Some(stamp);
            self.sysex_type(body);
            return;
        }
        let tcount = self.sysex_type(body);
        self.sysex.retain(|s| s.data != body);
        self.sysex.push(SysEx {
            data: body.to_vec(),
            finished: true,
            tcount,
            stamp,
        });
    }

    fn segment(&mut self, data: &[u8], first: bool, last: bool, stamp: Stamp) {
        let mut so_far = match (first, self.unfinished.take()) {
            (false, Some(u)) => u.data,
            _ => Vec::new(),
        };
        so_far.extend_from_slice(data);
        if last {
            self.finished_sysex(&so_far, stamp);
        } else {
            self.unfinished = Some(SysEx {
                data: so_far,
                finished: false,
                tcount: 0,
                stamp,
            });
        }
    }

    fn quarter_frame(&mut self, piece: u8, value: u8, stamp: Stamp) {
        let m = &mut self.mtc;
        m.stamp = Some(stamp);
        m.nibbles[piece as usize] = value & 0x0F;
        let continues = match (m.partial, m.last_piece) {
            (Some((reverse, _)), Some(last)) => {
                if reverse {
                    piece + 1 == last
                } else {
                    piece == last + 1
                }
            }
            _ => false,
        };
        m.last_piece = Some(piece);
        if piece == 0 && !continues {
            // A forward run starts.
            m.partial = Some((false, 0));
            m.reverse = false;
            m.nibbles = [0; 8];
            m.nibbles[0] = value & 0x0F;
            return;
        }
        if piece == 7 && !continues {
            m.partial = Some((true, 7));
            m.reverse = true;
            m.nibbles = [0; 8];
            m.nibbles[7] = value & 0x0F;
            return;
        }
        let Some((reverse, _)) = m.partial.filter(|_| continues) else {
            // Out of sequence: no partial frame value.
            m.partial = None;
            return;
        };
        let complete = if reverse { piece == 0 } else { piece == 7 };
        if !complete {
            m.partial = Some((reverse, piece));
            return;
        }
        m.partial = None;
        let mut nibbles = m.nibbles;
        if !reverse {
            // Forward, the pieces arrive two frames after the time they
            // carry: COMPLETE is coded two frames on (B.4).
            let n = &nibbles;
            if let Some(tc) = Timecode::new(
                n[6] | (n[7] & 1) << 4,
                n[4] | (n[5] & 3) << 4,
                n[2] | (n[3] & 3) << 4,
                n[0] | (n[1] & 1) << 4,
                Rate::from_code(n[7] >> 1),
            ) {
                let on = tc.add_frames(2);
                let keep = |old: u8, new: u8, mask: u8| (old & !mask) | (new & mask);
                nibbles = [
                    on.frames & 0x0F,
                    keep(n[1], on.frames >> 4, 1),
                    on.seconds & 0x0F,
                    keep(n[3], on.seconds >> 4, 3),
                    on.minutes & 0x0F,
                    keep(n[5], on.minutes >> 4, 3),
                    on.hours & 0x0F,
                    keep(n[7], on.hours >> 4, 1),
                ];
            }
        }
        m.complete = Some((nibbles, reverse));
        m.full = None;
        m.reverse = reverse;
    }

    /// The journal for packet `seq` (extended), sent at `now` (ms).
    pub(crate) fn encode(&self, seq: u64, now: u64) -> Vec<u8> {
        let checkpoint = if self.first.is_some() {
            self.checkpoint
        } else {
            seq
        };
        let previous = seq.checked_sub(1).filter(|p| Some(*p) == self.last_sent);
        let fresh = |s: Stamp| s.seq >= checkpoint;
        let mut out = vec![0u8, (checkpoint >> 8) as u8, checkpoint as u8];
        let mut s_top = 0x80;
        let system = self.system_journal(&fresh, previous);
        if let Some((bytes, s)) = &system {
            out.extend_from_slice(bytes);
            s_top &= s;
        }
        let mut count = 0u8;
        for (n, channel) in self.channels.iter().enumerate() {
            if let Some((bytes, s)) = channel_journal(n as u8, channel, &fresh, previous, now) {
                out.extend_from_slice(&bytes);
                s_top &= s;
                count += 1;
            }
        }
        out[0] = s_top
            | if system.is_some() { 0x40 } else { 0 }
            | if count > 0 { 0x20 | (count - 1) } else { 0 };
        out
    }

    fn system_journal(
        &self,
        fresh: &dyn Fn(Stamp) -> bool,
        previous: Option<u64>,
    ) -> Option<(Vec<u8>, u8)> {
        let mut toc = 0u8;
        let mut body = Vec::new();
        let mut s_all = 0x80u8;

        // Chapter D.
        let mut d = Vec::new();
        let mut d_toc = 0u8;
        let mut d_s = 0x80u8;
        if let Some(stamp) = self.reset_stamp.filter(|s| fresh(*s)) {
            d_toc |= 0x40;
            let s = s_bit(stamp, previous);
            d_s &= s;
            d.push(s | self.reset_count);
        }
        if let Some((count, stamp)) = self.tune.filter(|t| fresh(t.1)) {
            d_toc |= 0x20;
            let s = s_bit(stamp, previous);
            d_s &= s;
            d.push(s | count);
        }
        if let Some((song, stamp)) = self.song.filter(|t| fresh(t.1)) {
            d_toc |= 0x10;
            let s = s_bit(stamp, previous);
            d_s &= s;
            d.push(s | song);
        }
        for (i, (count, stamp)) in self.undefined.iter().enumerate() {
            let Some(stamp) = stamp.filter(|s| fresh(*s)) else {
                continue;
            };
            let s = s_bit(stamp, previous);
            d_s &= s;
            d_toc |= 0x08 >> i;
            if i < 2 {
                // C set, DSZ 0 (no data), LENGTH 3.
                d.extend_from_slice(&[s | 0x40, 3, *count]);
            } else {
                // C set, LENGTH 2.
                d.extend_from_slice(&[s | 0x40 | 2, *count]);
            }
        }
        if d_toc != 0 {
            toc |= 0x40;
            s_all &= d_s;
            body.push(d_s | d_toc);
            body.extend_from_slice(&d);
        }

        // Chapter V.
        if let Some(stamp) = self.sense.filter(|s| fresh(*s)) {
            toc |= 0x20;
            let s = s_bit(stamp, previous);
            s_all &= s;
            body.push(s | self.sense_count);
        }

        // Chapter Q.
        let q = &self.sequencer;
        if let Some(stamp) = q.stamp.filter(|s| fresh(*s)) {
            toc |= 0x10;
            let s = s_bit(stamp, previous);
            s_all &= s;
            // At the start of the song C is clear, unless a continue (not a
            // start) is about to play it (B.3).
            let coded = q.position != 0 || (q.running && !q.downbeat && !q.started);
            let mut head = s;
            if q.running {
                head |= 0x40;
            }
            if q.downbeat {
                head |= 0x20;
            }
            if coded {
                head |= 0x10 | (q.position >> 16) as u8 & 0x07;
            }
            body.push(head);
            if coded {
                body.extend_from_slice(&(q.position as u16).to_be_bytes());
            }
        }

        // Chapter F.
        let m = &self.mtc;
        if let Some(stamp) = m.stamp.filter(|s| fresh(*s)) {
            toc |= 0x08;
            let s = s_bit(stamp, previous);
            s_all &= s;
            let mut head = s;
            let mut fields = Vec::new();
            if let Some((nibbles, _)) = m.complete {
                head |= 0x40 | 0x10;
                fields.extend(pack_nibbles(&nibbles));
            } else if let Some(full) = m.full {
                head |= 0x40;
                fields.extend_from_slice(&full);
            }
            let reverse = m.reverse;
            if reverse {
                head |= 0x08;
            }
            match m.partial {
                Some((r, point)) => {
                    let range = if r { point..=7 } else { 0..=point };
                    let mut partial = [0u8; 8];
                    for i in range {
                        partial[i as usize] = m.nibbles[i as usize];
                    }
                    head |= 0x20 | point;
                    fields.extend(pack_nibbles(&partial));
                }
                None => head |= if reverse { 0 } else { 7 },
            }
            body.push(head);
            body.extend_from_slice(&fields);
        }

        // Chapter X: the logs that fit, the most recent kept.
        let mut logs: Vec<&SysEx> = self.sysex.iter().filter(|x| fresh(x.stamp)).collect();
        if let Some(u) = self.unfinished.as_ref().filter(|u| fresh(u.stamp)) {
            logs.push(u);
        }
        logs.sort_by_key(|x| x.stamp.order);
        let mut coded: Vec<Vec<u8>> = logs
            .iter()
            .map(|x| {
                let s = s_bit(x.stamp, previous);
                let sta = if x.finished { 3 } else { 0 };
                let has_data = !x.data.is_empty();
                let mut log = vec![
                    s | if x.finished { 0x40 } else { 0 } | if has_data { 0x08 } else { 0 } | sta,
                ];
                if x.finished {
                    log.push(x.tcount);
                }
                if has_data {
                    log.extend_from_slice(&x.data);
                    *log.last_mut().expect("data") |= 0x80;
                }
                log
            })
            .collect();
        let room = LENGTH_MAX.saturating_sub(2 + body.len());
        while coded.iter().map(Vec::len).sum::<usize>() > room {
            coded.remove(0);
        }
        if !coded.is_empty() {
            toc |= 0x04;
            // The first log's S bit stands for the chapter.
            let s_x = coded.iter().fold(0x80, |acc, l| acc & l[0] & 0x80);
            s_all &= s_x;
            coded[0][0] = (coded[0][0] & 0x7F) | s_x;
            for log in coded {
                body.extend(log);
            }
        }

        if toc == 0 {
            return None;
        }
        let length = body.len() + 2;
        let mut out = vec![s_all | toc | (length >> 8) as u8 & 0x03, length as u8];
        out.extend(body);
        Some((out, s_all))
    }
}

fn pack_nibbles(n: &[u8; 8]) -> [u8; 4] {
    [
        n[0] << 4 | n[1],
        n[2] << 4 | n[3],
        n[4] << 4 | n[5],
        n[6] << 4 | n[7],
    ]
}

/// A note on that went out this recently is "simultaneous" with the packet
/// (the Y bit, RFC 4696 §6): a receiver plays it rather than skip it.
const NOTE_RECENT_MS: u64 = 50;

fn channel_journal(
    channel: u8,
    c: &Channel,
    fresh: &dyn Fn(Stamp) -> bool,
    previous: Option<u64>,
    now: u64,
) -> Option<(Vec<u8>, u8)> {
    let mut toc = 0u8;
    let mut body = Vec::new();
    let s_all = std::cell::Cell::new(0x80u8);
    let s = |stamp: Stamp| {
        let s = s_bit(stamp, previous);
        s_all.set(s_all.get() & s);
        s
    };

    // P.
    if let Some((program, bank, stamp)) = c.program.filter(|p| fresh(p.2)) {
        toc |= 0x80;
        let (b, msb, lsb, x) = match bank {
            Some((msb, lsb, x)) => (0x80, msb, lsb, x),
            None => (0, 0, 0, false),
        };
        body.extend_from_slice(&[s(stamp) | program, b | msb, if x { 0x80 } else { 0 } | lsb]);
    }

    // C.
    let mut controllers: Vec<(u8, Controller)> = c
        .controllers
        .iter()
        .enumerate()
        .filter_map(|(n, x)| x.filter(|x| fresh(x.stamp)).map(|x| (n as u8, x)))
        .collect();
    if !controllers.is_empty() {
        toc |= 0x40;
        controllers.sort_by_key(|(_, x)| x.stamp.order);
        let mut logs = Vec::new();
        let mut s_c = 0x80;
        for (n, x) in &controllers {
            let sb = s(x.stamp);
            s_c &= sb;
            let alt = if count_tool(*n) {
                0xC0 | (x.count & 0x3F)
            } else {
                x.value & 0x7F
            };
            logs.extend_from_slice(&[sb | n, alt]);
        }
        body.push(s_c | (controllers.len() - 1) as u8);
        body.extend(logs);
    }

    // M.
    let mut params: Vec<(&(bool, u16), &Param)> =
        c.params.iter().filter(|(_, p)| fresh(p.stamp)).collect();
    let transaction_fresh = c.transaction.is_some_and(fresh);
    if !params.is_empty() || transaction_fresh {
        toc |= 0x20;
        params.sort_by_key(|(_, p)| p.stamp.order);
        let mut s_m = 0x80;
        if let Some(t) = c.transaction {
            if transaction_fresh {
                s_m &= s(t);
            }
        }
        let after_reset = |order: u64| c.reset_controllers.is_some_and(|r| order < r);
        let mut logs = Vec::new();
        for ((nrpn, number), p) in &params {
            let sb = s(p.stamp);
            s_m &= sb;
            let mut toc_log = 0x02 | 0x04; // V and T: both tools.
            let mut fields = Vec::new();
            if let Some((v, order)) = p.entry_msb {
                toc_log |= 0x80;
                fields.push(if after_reset(order) { 0x80 } else { 0 } | v);
            }
            if let Some((v, order)) = p.entry_lsb {
                toc_log |= 0x40;
                fields.push(if after_reset(order) { 0x80 } else { 0 } | v);
            }
            if let Some(order) = p.button_order {
                toc_log |= 0x20;
                let x = after_reset(order);
                let a = p.a_button;
                fields.extend_from_slice(&button(a, x));
                if !x && p.c_button != a {
                    toc_log |= 0x10;
                    fields.extend_from_slice(&button(p.c_button, false));
                }
            }
            toc_log |= 0x08;
            fields.push(if after_reset(p.count_order) { 0x80 } else { 0 } | p.count);
            logs.push(sb | (number & 0x7F) as u8);
            logs.push(if *nrpn { 0x80 } else { 0 } | (number >> 7) as u8);
            logs.push(toc_log);
            logs.extend(fields);
        }
        let pending = c.pending.then(|| c.msb.unwrap_or((false, 0)));
        let e = !c.pending && c.selected.is_some();
        let mut head = Vec::new();
        if let Some((nrpn, value)) = pending {
            head.push(if nrpn { 0x80 } else { 0 } | value);
        }
        let length = 2 + head.len() + logs.len();
        body.push(
            s_m | if pending.is_some() { 0x40 } else { 0 }
                | if e { 0x20 } else { 0 }
                | (length >> 8) as u8 & 0x03,
        );
        body.push(length as u8);
        body.extend(head);
        body.extend(logs);
    }

    // W.
    if let Some((bytes, stamp)) = c.wheel.filter(|w| fresh(w.1)) {
        toc |= 0x10;
        body.extend_from_slice(&[s(stamp) | bytes[0], bytes[1]]);
    }

    // N.
    let mut ons: Vec<(u8, Note)> = Vec::new();
    let mut offs: Vec<u8> = Vec::new();
    for (n, note) in c.notes.iter().enumerate() {
        if let Some(note) = note.filter(|x| fresh(x.stamp)) {
            if note.on {
                ons.push((n as u8, note));
            } else {
                offs.push(n as u8);
            }
        }
    }
    if !ons.is_empty() || !offs.is_empty() {
        toc |= 0x08;
        ons.sort_by_key(|(_, x)| x.stamp.order);
        let b = if c.last_note_off.is_some() && c.last_note_off == previous {
            s_all.set(0);
            0
        } else {
            0x80
        };
        let (low, high) = match (offs.first(), offs.last()) {
            (Some(lo), Some(hi)) => (lo / 8, hi / 8),
            _ if ons.len() == 128 => (15, 0),
            _ => (15, 1),
        };
        body.push(b | (ons.len().min(127)) as u8);
        body.push(low << 4 | high);
        for (n, note) in &ons {
            let y = if now.saturating_sub(note.at) <= NOTE_RECENT_MS {
                0x80
            } else {
                0
            };
            body.extend_from_slice(&[s(note.stamp) | n, y | note.velocity]);
        }
        if low <= high {
            let mut bits = vec![0u8; (high - low + 1) as usize];
            for n in &offs {
                bits[(n / 8 - low) as usize] |= 0x80 >> (n % 8);
            }
            body.extend(bits);
        }
    }

    // E.
    let mut extras: Vec<(u64, Stamp, u8, u8)> = Vec::new();
    for (n, note) in c.notes.iter().enumerate() {
        let Some(note) = note.filter(|x| fresh(x.stamp)) else {
            continue;
        };
        let refs = c.references[n];
        let counted = if note.on { refs > 1 } else { refs > 0 };
        if counted {
            extras.push((note.stamp.order, note.stamp, n as u8, refs.min(127) as u8));
        }
        if !note.on && note.velocity != 64 {
            extras.push((note.stamp.order, note.stamp, n as u8, 0x80 | note.velocity));
        }
    }
    if !extras.is_empty() {
        toc |= 0x04;
        extras.sort_by_key(|e| e.0);
        // At most 128 logs: release velocity logs go first.
        while extras.len() > 128 {
            match extras.iter().position(|e| e.3 & 0x80 != 0) {
                Some(i) => {
                    extras.remove(i);
                }
                None => break,
            }
        }
        extras.truncate(128);
        let mut s_e = 0x80;
        let mut logs = Vec::new();
        for (_, stamp, n, field) in &extras {
            let sb = s(*stamp);
            s_e &= sb;
            logs.extend_from_slice(&[sb | n, *field]);
        }
        body.push(s_e | (extras.len() - 1) as u8);
        body.extend(logs);
    }

    // T.
    if let Some((pressure, stamp)) = c.pressure.filter(|p| fresh(p.1)) {
        toc |= 0x02;
        body.push(s(stamp) | pressure);
    }

    // A.
    let mut poly: Vec<(u8, (u8, Stamp, bool))> = c
        .poly
        .iter()
        .enumerate()
        .filter_map(|(n, p)| p.filter(|p| fresh(p.1)).map(|p| (n as u8, p)))
        .collect();
    if !poly.is_empty() {
        toc |= 0x01;
        poly.sort_by_key(|(_, p)| p.1.order);
        let mut s_a = 0x80;
        let mut logs = Vec::new();
        for (n, (pressure, stamp, x)) in &poly {
            let sb = s(*stamp);
            s_a &= sb;
            logs.extend_from_slice(&[sb | n, if *x { 0x80 } else { 0 } | pressure]);
        }
        body.push(s_a | (poly.len() - 1) as u8);
        body.extend(logs);
    }

    if toc == 0 {
        return None;
    }
    let s_all = s_all.get();
    let length = body.len() + 3;
    let mut out = vec![
        s_all | channel << 3 | (length >> 8) as u8 & 0x03,
        length as u8,
        toc,
    ];
    out.extend(body);
    Some((out, s_all))
}

/// A-BUTTON or C-BUTTON: the sign, X (or R), and the magnitude.
fn button(count: i32, x: bool) -> [u8; 2] {
    let magnitude = count.unsigned_abs().min(16383) as u16;
    let g = if count < 0 { 0x80 } else { 0 };
    let x = if x { 0x40 } else { 0 };
    [g | x | (magnitude >> 8) as u8 & 0x3F, magnitude as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(items: &[Message]) -> Vec<Item> {
        items.iter().cloned().map(Item::Message).collect()
    }

    fn cc(channel: u8, controller: u8, value: u8) -> Message {
        Message::ControlChange {
            channel,
            controller,
            value,
        }
    }

    #[test]
    fn an_empty_history_is_an_empty_journal() {
        let j = Journal::new();
        assert_eq!(j.encode(100, 0), [0x80, 0, 100]);
    }

    #[test]
    fn channel_chapters_follow_the_history_and_the_checkpoint() {
        let mut j = Journal::new();
        j.record(
            10,
            0,
            &m(&[
                cc(1, 0, 1),
                cc(1, 32, 2),
                Message::ProgramChange {
                    channel: 1,
                    program: 5,
                },
                cc(1, 7, 90),
                Message::NoteOn {
                    channel: 1,
                    note: 60,
                    velocity: 100,
                },
                Message::NoteOn {
                    channel: 1,
                    note: 9,
                    velocity: 50,
                },
            ]),
        );
        j.record(
            11,
            1000,
            &m(&[
                Message::NoteOff {
                    channel: 1,
                    note: 9,
                    velocity: 64,
                },
                Message::PitchBend {
                    channel: 1,
                    value: 0x2000,
                },
                Message::ChannelPressure {
                    channel: 1,
                    pressure: 33,
                },
                Message::PolyPressure {
                    channel: 1,
                    note: 60,
                    pressure: 12,
                },
            ]),
        );
        let journal = j.encode(12, 1000);
        // Header: S clear (packet 11 is the one before), A, one channel;
        // checkpoint 10.
        assert_eq!(&journal[..3], &[0x20, 0, 10]);
        let channel = &journal[3..];
        assert_eq!(channel[0] & 0x78, 1 << 3);
        assert_eq!(channel[2], 0b1101_1011); // P C W N T A
        let mut chapters = &channel[3..];
        // P: program 5, bank 1/2.
        assert_eq!(&chapters[..3], &[0x80 | 5, 0x80 | 1, 2]);
        chapters = &chapters[3..];
        // C: three logs, oldest first: 0, 32, 7.
        assert_eq!(chapters[0] & 0x7F, 2);
        assert_eq!(&chapters[1..7], &[0x80, 1, 0x80 | 32, 2, 0x80 | 7, 90]);
        chapters = &chapters[7..];
        // W, S clear.
        assert_eq!(&chapters[..2], &[0x00, 0x40]);
        chapters = &chapters[2..];
        // N: note 60 on (not recent: Y clear), note 9 off; B clear as the
        // note off was in packet 11.
        assert_eq!(&chapters[..2], &[0x01, 0x11]);
        assert_eq!(&chapters[2..4], &[0x80 | 60, 100]);
        assert_eq!(chapters[4], 0x40);
        chapters = &chapters[5..];
        // T and A.
        assert_eq!(chapters[0], 33);
        assert_eq!(&chapters[1..4], &[0x00, 60, 12]);
        assert_eq!(
            channel.len(),
            ((channel[0] as usize & 3) << 8) | channel[1] as usize
        );

        // The peer has packet 11: the checkpoint moves to 12, and the
        // journal is empty again.
        j.acknowledge(11);
        assert!(j.acknowledged(11));
        assert!(!j.acknowledged(12));
        assert_eq!(j.encode(12, 1000), [0x80, 0, 12]);
    }

    #[test]
    fn the_receiver_replays_what_the_journal_codes() {
        let mut j = Journal::new();
        j.record(
            1,
            0,
            &m(&[
                cc(2, 0, 1),
                cc(2, 32, 2),
                Message::ProgramChange {
                    channel: 2,
                    program: 5,
                },
                cc(2, 7, 80),
                Message::PitchBend {
                    channel: 2,
                    value: 0x2000,
                },
                Message::NoteOn {
                    channel: 2,
                    note: 64,
                    velocity: 90,
                },
                Message::ChannelPressure {
                    channel: 2,
                    pressure: 33,
                },
            ]),
        );
        let journal = j.encode(2, 10);
        let mut out = Vec::new();
        super::super::rtp_midi::recover_for_tests(&journal, &mut out);
        assert_eq!(
            out,
            [
                0xB2, 0x00, 1, 0xB2, 0x20, 2, 0xC2, 5, // P
                0xB2, 0, 1, 0xB2, 32, 2, 0xB2, 7, 80, // C
                0xE2, 0x00, 0x40, // W
                0x92, 64, 90, // N
                0xD2, 33, // T
            ]
        );
    }

    #[test]
    fn controllers_use_their_tools_and_pairs_keep_the_latest() {
        let mut j = Journal::new();
        j.record(
            1,
            0,
            &m(&[
                cc(0, 123, 0),
                cc(0, 123, 0),
                cc(0, 124, 0),
                cc(0, 125, 0),
                cc(0, 64, 127),
            ]),
        );
        let journal = j.encode(5, 0);
        let c = &journal[6..];
        // Three logs: 123 (count 2), 125 (count 1; 124 dropped), 64 = 127.
        assert_eq!(c[0], 0x80 | 2);
        assert_eq!(
            &c[1..7],
            &[0x80 | 123, 0xC0 | 2, 0x80 | 125, 0xC0 | 1, 0x80 | 64, 127]
        );
    }

    #[test]
    fn notes_off_and_resets_end_what_is_active() {
        let mut j = Journal::new();
        j.record(
            1,
            0,
            &m(&[
                Message::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                },
                Message::ChannelPressure {
                    channel: 0,
                    pressure: 10,
                },
                cc(0, 123, 0),
            ]),
        );
        let journal = j.encode(3, 0);
        // Only chapter C remains (the all notes off).
        assert_eq!(journal[5], 0x40);
        j.record(2, 0, &m(&[Message::Reset]));
        let journal = j.encode(3, 0);
        // A system journal with chapter D's reset log, and no channel.
        assert_eq!(journal[0] & 0x60, 0x40);
        assert_eq!(&journal[3..6], &[0x40, 4, 0x40]);
        assert_eq!(journal[6], 1);
    }

    #[test]
    fn rpn_and_nrpn_go_in_chapter_m() {
        let mut j = Journal::new();
        j.record(
            1,
            0,
            &m(&[cc(0, 99, 2), cc(0, 98, 44), cc(0, 6, 10), cc(0, 38, 5)]),
        );
        let journal = j.encode(3, 0);
        let channel = &journal[3..];
        assert_eq!(channel[2], 0x20, "only chapter M");
        let chapter = &channel[3..];
        // S, E set (a transaction in progress), LENGTH.
        assert_eq!(chapter[0], 0x80 | 0x20);
        assert_eq!(chapter[1] as usize, chapter.len());
        // The log: parameter 300 (2/44) as NRPN, J K N with V and T, then
        // ENTRY-MSB, ENTRY-LSB and COUNT.
        assert_eq!(&chapter[2..], &[0x80 | 44, 0x80 | 2, 0xCE, 10, 5, 1]);

        // Deselected with the null parameter: E clear, the log kept.
        j.record(2, 0, &m(&[cc(0, 101, 127), cc(0, 100, 127)]));
        let journal = j.encode(3, 0);
        let chapter = &journal[6..];
        assert_eq!(chapter[0] & 0x60, 0);
        // A pending MSB: P set and the PENDING octet (RPN, so Q clear).
        j.record(3, 0, &m(&[cc(0, 101, 0)]));
        let journal = j.encode(4, 0);
        let chapter = &journal[6..];
        assert_eq!(chapter[0] & 0x60, 0x40);
        assert_eq!(chapter[2], 0);
        // With no transaction open, data entry is a general controller.
        let mut j = Journal::new();
        j.record(1, 0, &m(&[cc(0, 6, 3)]));
        assert_eq!(j.encode(2, 0)[5], 0x40);
    }

    #[test]
    fn system_chapters_code_the_sequencer_mtc_and_sysex() {
        let mut j = Journal::new();
        let mut items = m(&[
            Message::SongSelect(3),
            Message::TuneRequest,
            Message::ActiveSensing,
            Message::SongPosition(4),
            Message::Continue,
            Message::Clock,
            Message::Clock,
            Message::SysEx(vec![0x7F, 0x7F, 0x01, 0x01, 0x21, 2, 3, 4]),
            Message::SysEx(vec![0x43, 0x10]),
        ]);
        items.push(Item::Segment {
            data: vec![0x41, 0x42],
            first: true,
            last: false,
        });
        j.record(1, 0, &items);
        let journal = j.encode(5, 0);
        assert_eq!(journal[0] & 0x60, 0x40);
        let system = &journal[3..];
        let length = ((system[0] as usize & 3) << 8) | system[1] as usize;
        assert_eq!(length, system.len());
        // D V Q F X.
        assert_eq!(system[0] & 0x7C, 0x7C);
        let mut c = &system[2..];
        // D: tune request count 1, song 3.
        assert_eq!(&c[..3], &[0x80 | 0x30, 0x80 | 1, 0x80 | 3]);
        c = &c[3..];
        // V: one active sensing.
        assert_eq!(c[0], 0x80 | 1);
        c = &c[1..];
        // Q: running, downbeat played, position 24 + 1 clocks.
        assert_eq!(&c[..3], &[0x80 | 0x40 | 0x20 | 0x10, 0, 25]);
        c = &c[3..];
        // F: a full message, COMPLETE with Q clear, no partial (POINT 7).
        assert_eq!(&c[..5], &[0x80 | 0x40 | 7, 0x21, 2, 3, 4]);
        c = &c[5..];
        // X: the finished SysEx (T set, TCOUNT 1, STA 3), then the
        // unfinished one (STA 0).
        assert_eq!(&c[..4], &[0x80 | 0x40 | 0x08 | 3, 1, 0x43, 0x90]);
        assert_eq!(&c[4..], &[0x80 | 0x08, 0x41, 0xC2]);
    }

    #[test]
    fn quarter_frames_code_a_complete_time_two_frames_on_and_a_partial_one() {
        let tc = Timecode::new(1, 2, 3, 4, Rate::Fps25).unwrap();
        let mut items = m(&crate::midi::quarter_frames(&tc));
        items.extend(m(&[
            Message::QuarterFrame { piece: 0, value: 6 },
            Message::QuarterFrame { piece: 1, value: 0 },
        ]));
        let mut j = Journal::new();
        j.record(1, 0, &items);
        let journal = j.encode(2, 0);
        let f = &journal[5..];
        // C P Q, forward, POINT 1.
        assert_eq!(f[0], 0x40 | 0x20 | 0x10 | 1);
        // COMPLETE: 01:02:03:06 in nibbles (frames two on), rate 25 in MT7.
        assert_eq!(&f[1..5], &[0x60, 0x30, 0x20, 0x12]);
        assert_eq!(&f[5..9], &[0x60, 0, 0, 0]);
    }

    #[test]
    fn segments_and_large_sysex_stay_within_the_system_journal() {
        let mut j = Journal::new();
        j.record(
            1,
            0,
            &[Item::Segment {
                data: vec![1; 997],
                first: true,
                last: false,
            }],
        );
        j.record(
            2,
            0,
            &[Item::Segment {
                data: vec![2; 600],
                first: false,
                last: true,
            }],
        );
        let journal = j.encode(3, 0);
        // The finished SysEx of 1597 bytes cannot be coded: no system
        // journal, and the journal stays valid.
        assert_eq!(journal, [0x80, 0, 1]);
        j.record(3, 0, &m(&[Message::SysEx(vec![5; 10])]));
        let journal = j.encode(4, 0);
        assert_eq!(journal[0] & 0x40, 0x40);
    }
}
