//! Generic MIDI: send any MIDI 1.0 message to a device over an RTP-MIDI
//! session or a MIDI port of the host (`super::midi_link`), and keep what it
//! sends.
//!
//! Messages from the MIDI 1.0 Detailed Specification 4.2.1 (MMA): channel
//! voice and mode messages, NRPN and RPN (controllers 99/98 and 101/100
//! select the parameter, 6 and 38 carry its value's MSB and LSB, 127/127
//! deselects), system common and real-time messages, and SysEx. MIDI Time
//! Code (MMA 4.2.1): quarter frames, assembled into the time once eight have
//! arrived in a row, and the full message and user bits (universal real-time
//! SysEx, sub-ID 01). MIDI Machine Control 1.0 (MMA RP-013): commands
//! (universal real-time SysEx, sub-ID 06) and responses (sub-ID 07).
//!
//! Every message received is kept in state (the last value of each
//! controller, note, program, pressure, pitch bend and NRPN or RPN per
//! channel; the MTC position; the last MMC command, locate point and
//! response fields; the song position and whether the sequencer runs) and
//! reported as a message event, except that MIDI clock, active sensing and
//! MTC quarter frames are reported only with `realtime_events`, as they
//! arrive many times a second.
//!
//! The core also generates MIDI Time Code (`mtc_generator_*`): running, it
//! sends the eight quarter frames of every two frames as they fall due by
//! its own clock (MTC 4.2.1: four a frame, piece 0 when the time they carry
//! begins, frames counted in drop-frame at 30df), at 24, 25, 29.97 (30df) or
//! 30 frames a second, and a locate sends the full message. A core that
//! falls more than a run of eight behind skips whole runs rather than send
//! them in a burst.
//!
//! MIDI carries no acknowledgement, so every command is `unverified`.
//! Opening for commands only changes nothing: the module asks the device for
//! nothing, and an RTP-MIDI session's clock synchronisation is part of
//! keeping the session, not monitoring.

use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use super::midi_link::{timecode, MidiLink};
use crate::catalog::Params;
use crate::midi::{self, Message, MtcReader, Rate, Timecode};
use crate::module::{
    CommandError, CommandId, Cx, Key, Module, OpenContext, Outcome, SettingsUpdate, TcpInput,
};

const MTC_IDLE: Key = "mtc-idle";
const MTC_GENERATOR: Key = "mtc-generator";
/// MTC is taken to have stopped when no run of quarter frames completes for
/// this long (a run takes two frames: 83 ms at 24 fps).
const MTC_IDLE_MS: u64 = 500;

/// The parameter a channel's data entry controllers write.
#[derive(Debug, Default, Clone, Copy)]
struct Selection {
    rpn: bool,
    msb: Option<u8>,
    lsb: Option<u8>,
    data_msb: Option<u8>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Channel {
    selection: Selection,
    bank_msb: Option<u8>,
    bank_lsb: Option<u8>,
}

/// The MTC generator: where it is and, running, since when.
#[derive(Debug, Clone, Copy)]
struct Generator {
    /// The time the current run of eight quarter frames carries.
    position: Timecode,
    running: bool,
    /// When quarter frame 0 of the run started at `position` was due, and
    /// how many have been sent since.
    started_at: u64,
    sent: u64,
    /// The time last reported in state.
    reported: Option<Timecode>,
}

impl Generator {
    fn new() -> Generator {
        Generator {
            position: Timecode::new(0, 0, 0, 0, Rate::Fps30).expect("midnight"),
            running: false,
            started_at: 0,
            sent: 0,
            reported: None,
        }
    }

    /// Frames a second as a fraction: 30df runs at 30000/1001.
    fn rate(&self) -> (u64, u64) {
        match self.position.rate {
            Rate::Fps24 => (24, 1),
            Rate::Fps25 => (25, 1),
            Rate::Fps30Drop => (30_000, 1001),
            Rate::Fps30 => (30, 1),
        }
    }

    /// When quarter frame `k` of this run falls due (ms).
    fn due(&self, k: u64) -> u64 {
        let (num, den) = self.rate();
        self.started_at + k * 250 * den / num
    }

    /// How many quarter frames of this run are due by `now`: the k with
    /// due(k) <= now.
    fn count_due(&self, now: u64) -> u64 {
        let (num, den) = self.rate();
        let Some(elapsed) = now.checked_sub(self.started_at) else {
            return 0;
        };
        ((elapsed + 1) * num - 1) / (250 * den) + 1
    }

    /// The frame the generator is at: the run's time, one frame on after
    /// four quarter frames.
    fn frame(&self) -> Timecode {
        if self.running {
            self.position.add_frames((self.sent % 8 / 4) as i64)
        } else {
            self.position
        }
    }

    fn state(&self) -> Value {
        let mut v = self.frame().json();
        v["running"] = json!(self.running);
        json!({"mtc_generator": v})
    }
}

pub(crate) struct GenericMidi {
    link: MidiLink,
    generator: Generator,
    device_id: u8,
    realtime_events: bool,
    channels: [Channel; 16],
    notes_on: [[bool; 128]; 16],
    mtc: MtcReader,
    /// The last full MMC time code field each device sent, for its short
    /// form.
    mmc_times: std::collections::HashMap<(u8, u8), [u8; 5]>,
    received: u64,
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| CommandError::InvalidParams {
            message: format!("{name} is required"),
        })
}

fn opt_int(params: &Params, name: &str) -> Option<i64> {
    params.get(name).and_then(Value::as_i64)
}

fn byte(params: &Params, name: &str) -> Result<u8, CommandError> {
    let v = int(params, name)?;
    u8::try_from(v)
        .ok()
        .filter(|b| *b < 0x80)
        .ok_or_else(|| CommandError::InvalidParams {
            message: format!("{name} {v} is not 0 to 127"),
        })
}

fn text<'a>(params: &'a Params, name: &str) -> Option<&'a str> {
    params.get(name).and_then(Value::as_str)
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// The channel parameter (1 to 16) as the wire's 0 to 15.
fn channel(params: &Params) -> Result<u8, CommandError> {
    let c = int(params, "channel")?;
    if !(1..=16).contains(&c) {
        return Err(invalid(format!("channel {c} is not 1 to 16")));
    }
    Ok(c as u8 - 1)
}

/// The 14-bit parameter numbers MMC reads and writes as time code, by name
/// (MMC 1.0, the Information Field index).
const MMC_TIME_FIELDS: [(u8, &str); 15] = [
    (0x01, "selected_time_code"),
    (0x02, "selected_master_code"),
    (0x03, "requested_offset"),
    (0x04, "actual_offset"),
    (0x05, "lock_deviation"),
    (0x06, "generator_time_code"),
    (0x07, "mtc_input"),
    (0x08, "gp0"),
    (0x09, "gp1"),
    (0x0A, "gp2"),
    (0x0B, "gp3"),
    (0x0C, "gp4"),
    (0x0D, "gp5"),
    (0x0E, "gp6"),
    (0x0F, "gp7"),
];

const MMC_MOTION_CONTROL_TALLY: u8 = 0x48;
const MMC_VELOCITY_TALLY: u8 = 0x49;

fn mmc_field_code(name: &str) -> Option<u8> {
    MMC_TIME_FIELDS
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(c, _)| *c)
        .or(match name {
            "motion_control_tally" => Some(MMC_MOTION_CONTROL_TALLY),
            "velocity_tally" => Some(MMC_VELOCITY_TALLY),
            _ => None,
        })
}

fn mmc_field_name(code: u8) -> String {
    MMC_TIME_FIELDS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| match code {
            MMC_MOTION_CONTROL_TALLY => "motion_control_tally".into(),
            MMC_VELOCITY_TALLY => "velocity_tally".into(),
            other => format!("{other:02x}"),
        })
}

/// One MMC response field's value (MMC 1.0, Information Fields).
fn mmc_field_value(code: u8, data: &[u8]) -> Value {
    match code {
        0x01..=0x0F => {
            midi::read_standard_time(data).unwrap_or_else(|| json!({"hex": midi::to_hex(data)}))
        }
        MMC_MOTION_CONTROL_TALLY if data.len() >= 3 => json!({
            "state": midi::mmc_name(data[0]),
            "process": if data[1] == 0x7F { "none".to_string() } else { midi::mmc_name(data[1]) },
            "state_success": data[2] & 0x07,
            "process_success": (data[2] >> 4) & 0x07,
        }),
        MMC_VELOCITY_TALLY => midi::mmc_speed_value(data)
            .map(|v| json!(v))
            .unwrap_or_else(|| json!({"hex": midi::to_hex(data)})),
        _ => json!({"hex": midi::to_hex(data)}),
    }
}

impl GenericMidi {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericMidi, String> {
        let link = MidiLink::from_context(&ctx)?;
        let device_id = ctx
            .settings
            .get("device_id")
            .and_then(Value::as_u64)
            .unwrap_or(127)
            .min(127) as u8;
        let realtime_events = ctx
            .settings
            .get("realtime_events")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(GenericMidi {
            link,
            generator: Generator::new(),
            device_id,
            realtime_events,
            channels: [Channel::default(); 16],
            notes_on: [[false; 128]; 16],
            mtc: MtcReader::default(),
            mmc_times: Default::default(),
            received: 0,
        })
    }

    fn device(&self, params: &Params) -> Result<u8, CommandError> {
        match opt_int(params, "device_id") {
            Some(_) => byte(params, "device_id"),
            None => Ok(self.device_id),
        }
    }

    /// A universal real-time SysEx to `device`.
    fn universal(device: u8, sub: u8, data: &[u8]) -> Message {
        let mut body = vec![midi::REAL_TIME, device, sub];
        body.extend_from_slice(data);
        Message::SysEx(body)
    }

    fn mmc(&self, params: &Params, data: &[u8]) -> Result<Vec<Message>, CommandError> {
        Ok(vec![GenericMidi::universal(
            self.device(params)?,
            midi::SUB_MMC_COMMAND,
            data,
        )])
    }

    /// The messages a command sends.
    fn messages(&self, name: &str, params: &Params) -> Result<Vec<Message>, CommandError> {
        let m = match name {
            "note_on" => vec![Message::NoteOn {
                channel: channel(params)?,
                note: byte(params, "note")?,
                velocity: byte(params, "velocity")?,
            }],
            "note_off" => vec![Message::NoteOff {
                channel: channel(params)?,
                note: byte(params, "note")?,
                velocity: byte(params, "velocity")?,
            }],
            "control_change" => vec![Message::ControlChange {
                channel: channel(params)?,
                controller: byte(params, "controller")?,
                value: byte(params, "value")?,
            }],
            "control_change_14bit" => {
                let ch = channel(params)?;
                let controller = byte(params, "controller")?;
                if controller > 31 {
                    return Err(invalid(
                        "controller must be 0 to 31: its LSB is controller + 32",
                    ));
                }
                let value = int(params, "value")?.clamp(0, 16383) as u16;
                vec![
                    Message::ControlChange {
                        channel: ch,
                        controller,
                        value: (value >> 7) as u8,
                    },
                    Message::ControlChange {
                        channel: ch,
                        controller: controller + 32,
                        value: (value & 0x7F) as u8,
                    },
                ]
            }
            "program_change" => {
                let ch = channel(params)?;
                let mut out = Vec::new();
                if opt_int(params, "bank_msb").is_some() {
                    out.push(Message::ControlChange {
                        channel: ch,
                        controller: 0,
                        value: byte(params, "bank_msb")?,
                    });
                }
                if opt_int(params, "bank_lsb").is_some() {
                    out.push(Message::ControlChange {
                        channel: ch,
                        controller: 32,
                        value: byte(params, "bank_lsb")?,
                    });
                }
                out.push(Message::ProgramChange {
                    channel: ch,
                    program: byte(params, "program")?,
                });
                out
            }
            "pitch_bend" => {
                let v = int(params, "value")?.clamp(-8192, 8191);
                vec![Message::PitchBend {
                    channel: channel(params)?,
                    value: (v + 8192) as u16,
                }]
            }
            "channel_pressure" => vec![Message::ChannelPressure {
                channel: channel(params)?,
                pressure: byte(params, "pressure")?,
            }],
            "poly_pressure" => vec![Message::PolyPressure {
                channel: channel(params)?,
                note: byte(params, "note")?,
                pressure: byte(params, "pressure")?,
            }],
            "nrpn" | "rpn" => {
                let ch = channel(params)?;
                let parameter = int(params, "parameter")?.clamp(0, 16383) as u16;
                let (msb_cc, lsb_cc) = if name == "nrpn" { (99, 98) } else { (101, 100) };
                let cc = |controller: u8, value: u8| Message::ControlChange {
                    channel: ch,
                    controller,
                    value,
                };
                let mut out = vec![
                    cc(msb_cc, (parameter >> 7) as u8),
                    cc(lsb_cc, (parameter & 0x7F) as u8),
                    cc(6, byte(params, "value_msb")?),
                ];
                if opt_int(params, "value_lsb").is_some() {
                    out.push(cc(38, byte(params, "value_lsb")?));
                }
                if params.get("deselect").and_then(Value::as_bool) == Some(true) {
                    // The RPN null (127/127) leaves no parameter selected.
                    out.push(cc(101, 127));
                    out.push(cc(100, 127));
                }
                out
            }
            "channel_mode" => {
                let ch = channel(params)?;
                let (controller, value) = match text(params, "mode").unwrap_or_default() {
                    "all_sound_off" => (120, 0),
                    "reset_all_controllers" => (121, 0),
                    "local_control_off" => (122, 0),
                    "local_control_on" => (122, 127),
                    "all_notes_off" => (123, 0),
                    "omni_off" => (124, 0),
                    "omni_on" => (125, 0),
                    "mono_on" => (126, byte(params, "channels")?.min(16)),
                    "poly_on" => (127, 0),
                    other => return Err(invalid(format!("mode '{other}' is not a channel mode"))),
                };
                vec![Message::ControlChange {
                    channel: ch,
                    controller,
                    value,
                }]
            }
            "sysex" => {
                let hex = text(params, "hex").unwrap_or_default();
                let mut bytes = midi::parse_hex(hex).map_err(invalid)?;
                if bytes.first() == Some(&0xF0) {
                    bytes.remove(0);
                }
                if bytes.last() == Some(&0xF7) {
                    bytes.pop();
                }
                if let Some(b) = bytes.iter().find(|b| **b >= 0x80) {
                    return Err(invalid(format!(
                        "{b:02x} is a status byte: SysEx data bytes are 00 to 7F"
                    )));
                }
                vec![Message::SysEx(bytes)]
            }
            "raw" => {
                let bytes =
                    midi::parse_hex(text(params, "hex").unwrap_or_default()).map_err(invalid)?;
                // Whatever does not complete a message is refused rather than
                // sent half.
                if !ends_whole(&bytes) {
                    return Err(invalid(
                        "the bytes do not end on a whole MIDI message (a SysEx needs its F7)",
                    ));
                }
                let messages = midi::Parser::default().feed(&bytes);
                if messages.is_empty() {
                    return Err(invalid("no MIDI message in the bytes"));
                }
                messages
            }
            "song_position" => vec![Message::SongPosition(
                int(params, "beats")?.clamp(0, 16383) as u16
            )],
            "song_select" => vec![Message::SongSelect(byte(params, "song")?)],
            "tune_request" => vec![Message::TuneRequest],
            "realtime" => vec![match text(params, "message").unwrap_or_default() {
                "clock" => Message::Clock,
                "start" => Message::Start,
                "continue" => Message::Continue,
                "stop" => Message::Stop,
                "active_sensing" => Message::ActiveSensing,
                "system_reset" => Message::Reset,
                other => return Err(invalid(format!("'{other}' is not a real-time message"))),
            }],
            "mtc_quarter_frame" => vec![Message::QuarterFrame {
                piece: int(params, "piece")?.clamp(0, 7) as u8,
                value: int(params, "value")?.clamp(0, 15) as u8,
            }],
            "mtc_full_frame" => {
                let tc = timecode(params)?;
                vec![GenericMidi::universal(
                    self.device(params)?,
                    midi::SUB_MTC,
                    &[0x01, tc.hours_byte(), tc.minutes, tc.seconds, tc.frames],
                )]
            }
            "mtc_user_bits" => {
                let bits = text(params, "user_bits").unwrap_or_default();
                if bits.len() != 8 || !bits.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(invalid(
                        "user_bits is eight hex digits, binary group 8 first",
                    ));
                }
                // Group 8 is written first and group 1 last (MIDI Time Code,
                // "User Bits"); u1 carries group 1.
                let mut data = vec![0x02];
                for c in bits.chars().rev() {
                    data.push(c.to_digit(16).unwrap_or(0) as u8);
                }
                data.push(opt_int(params, "flags").unwrap_or(0).clamp(0, 3) as u8);
                vec![GenericMidi::universal(
                    self.device(params)?,
                    midi::SUB_MTC,
                    &data,
                )]
            }
            "mmc" => {
                let command = text(params, "command").unwrap_or_default();
                let code = midi::MMC_COMMANDS
                    .iter()
                    .find(|(_, n)| *n == command)
                    .map(|(c, _)| *c)
                    .ok_or_else(|| invalid(format!("'{command}' is not an MMC command")))?;
                self.mmc(params, &[code])?
            }
            "mmc_locate" => {
                let tc = timecode(params)?;
                let sub = opt_int(params, "subframes").unwrap_or(0).clamp(0, 99) as u8;
                let mut data = vec![0x44, 0x06, 0x01];
                data.extend_from_slice(&midi::standard_time(&tc, sub));
                self.mmc(params, &data)?
            }
            "mmc_locate_register" => {
                let register = int(params, "register")?.clamp(0, 7) as u8;
                self.mmc(params, &[0x44, 0x02, 0x00, 0x08 + register])?
            }
            "mmc_variable_play" | "mmc_search" | "mmc_shuttle" => {
                let speed = params
                    .get("speed")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| invalid("speed is required"))?;
                let coded = midi::mmc_speed(speed).ok_or_else(|| {
                    invalid(format!(
                        "speed {speed} is beyond MMC's ±1023.99 times play speed"
                    ))
                })?;
                let code = match name {
                    "mmc_variable_play" => 0x45,
                    "mmc_search" => 0x46,
                    _ => 0x47,
                };
                self.mmc(params, &[code, 0x03, coded[0], coded[1], coded[2]])?
            }
            "mmc_step" => {
                let steps = int(params, "steps")?.clamp(-63, 63);
                let coded = if steps < 0 {
                    0x40 | (-steps) as u8
                } else {
                    steps as u8
                };
                self.mmc(params, &[0x48, 0x01, coded])?
            }
            "mmc_read" => {
                let field = text(params, "field").unwrap_or_default();
                let code = mmc_field_code(field)
                    .ok_or_else(|| invalid(format!("'{field}' is not an MMC field this reads")))?;
                self.mmc(params, &[0x42, 0x01, code])?
            }
            "mmc_write_time" => {
                let field = text(params, "field").unwrap_or_default();
                let code = MMC_TIME_FIELDS
                    .iter()
                    .find(|(_, n)| *n == field)
                    .map(|(c, _)| *c)
                    .ok_or_else(|| invalid(format!("'{field}' is not a time field")))?;
                let tc = timecode(params)?;
                let sub = opt_int(params, "subframes").unwrap_or(0).clamp(0, 99) as u8;
                let mut data = vec![0x40, 0x06, code];
                data.extend_from_slice(&midi::standard_time(&tc, sub));
                self.mmc(params, &data)?
            }
            other => {
                return Err(CommandError::UnknownCommand {
                    command: other.into(),
                })
            }
        };
        Ok(m)
    }

    /// The time a generator command gives: its fields (0 when left out) at
    /// its rate, or the generator's rate when it gives none.
    fn generator_time(&self, params: &Params) -> Result<Timecode, CommandError> {
        let mut params = params.clone();
        if !params.contains_key("rate") {
            params.insert("rate".into(), json!(self.generator.position.rate.name()));
        }
        timecode(&params)
    }

    /// Move the generator to `tc`, restarting its run of quarter frames now.
    fn generator_move(&mut self, cx: &mut Cx, tc: Timecode) {
        let g = &mut self.generator;
        g.position = tc;
        g.started_at = cx.now();
        g.sent = 0;
        g.reported = None;
    }

    /// Send every quarter frame due by now, and wait for the next.
    fn generator_run(&mut self, cx: &mut Cx) {
        if !self.generator.running {
            return;
        }
        let now = cx.now();
        let mut messages = Vec::new();
        let g = &mut self.generator;
        // Far behind (the core was busy): skip whole runs of eight.
        let due = g.count_due(now).saturating_sub(g.sent);
        if due > 8 {
            let skip = (due - 1) / 8 * 8;
            let runs = (g.sent % 8 + skip) / 8;
            g.position = g.position.add_frames(2 * runs as i64);
            g.sent += skip;
        }
        while g.due(g.sent) <= now {
            let piece = (g.sent % 8) as usize;
            messages.push(midi::quarter_frames(&g.position)[piece].clone());
            g.sent += 1;
            if g.sent.is_multiple_of(8) {
                g.position = g.position.add_frames(2);
            }
        }
        let next = g.due(g.sent).saturating_sub(now).max(1);
        cx.set_timer(MTC_GENERATOR, next);
        if !messages.is_empty() {
            // Sent while the link is up; time runs on either way.
            let _ = self.link.send(cx, &messages);
        }
        let frame = self.generator.frame();
        let report = self.generator.reported.is_none_or(|r| {
            (r.hours, r.minutes, r.seconds) != (frame.hours, frame.minutes, frame.seconds)
        });
        if report {
            self.generator.reported = Some(frame);
            cx.state(self.generator.state());
        }
    }

    /// The generator's commands; `None` for any other.
    fn generator_command(
        &mut self,
        cx: &mut Cx,
        name: &str,
        params: &Params,
    ) -> Option<Result<Outcome, CommandError>> {
        let result = match name {
            "mtc_generator_start" => {
                let given = ["hours", "minutes", "seconds", "frames", "rate"]
                    .iter()
                    .any(|k| params.contains_key(*k));
                let tc = if given {
                    match self.generator_time(params) {
                        Ok(tc) => tc,
                        Err(e) => return Some(Err(e)),
                    }
                } else {
                    self.generator.frame()
                };
                self.generator_move(cx, tc);
                self.generator.running = true;
                self.generator_run(cx);
                Ok(Outcome::Unverified)
            }
            "mtc_generator_stop" => {
                let at = self.generator.frame();
                self.generator.running = false;
                self.generator.position = at;
                cx.cancel_timer(MTC_GENERATOR);
                cx.state(self.generator.state());
                Ok(Outcome::Unverified)
            }
            "mtc_generator_locate" => {
                let tc = match self.generator_time(params) {
                    Ok(tc) => tc,
                    Err(e) => return Some(Err(e)),
                };
                let device = match self.device(params) {
                    Ok(d) => d,
                    Err(e) => return Some(Err(e)),
                };
                let full = GenericMidi::universal(
                    device,
                    midi::SUB_MTC,
                    &[0x01, tc.hours_byte(), tc.minutes, tc.seconds, tc.frames],
                );
                let sent = self.link.send(cx, &[full]);
                self.generator_move(cx, tc);
                if self.generator.running {
                    self.generator_run(cx);
                } else {
                    cx.state(self.generator.state());
                }
                sent.map(|()| Outcome::Unverified)
            }
            _ => return None,
        };
        Some(result)
    }

    fn state_channel(&self, channel: u8, patch: Value) -> Value {
        json!({"channels": {(channel + 1).to_string(): patch}})
    }

    /// Keep what a received message says, and report it.
    fn receive(&mut self, cx: &mut Cx, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let source = self.link.source();
        for m in &messages {
            self.received += 1;
            self.apply(cx, m, source);
        }
        cx.state(json!({"received_count": self.received}));
    }

    fn event(&self, cx: &mut Cx, address: &str, args: Value, source: SocketAddr) {
        cx.message(address, None, args, source);
    }

    fn apply(&mut self, cx: &mut Cx, m: &Message, source: SocketAddr) {
        let (kind, args) = m.describe();
        match *m {
            Message::NoteOn {
                channel,
                note,
                velocity,
            } => {
                self.notes_on[channel as usize][note as usize] = velocity > 0;
                let patch = json!({"notes": {note.to_string(): {"on": velocity > 0, "velocity": velocity}}});
                cx.state(self.state_channel(channel, patch));
            }
            Message::NoteOff {
                channel,
                note,
                velocity,
            } => {
                self.notes_on[channel as usize][note as usize] = false;
                let patch =
                    json!({"notes": {note.to_string(): {"on": false, "velocity": velocity}}});
                cx.state(self.state_channel(channel, patch));
            }
            Message::PolyPressure {
                channel,
                note,
                pressure,
            } => {
                let patch = json!({"poly_pressure": {note.to_string(): pressure}});
                cx.state(self.state_channel(channel, patch));
            }
            Message::ControlChange {
                channel,
                controller,
                value,
            } => self.control_change(cx, channel, controller, value),
            Message::ProgramChange { channel, program } => {
                let c = self.channels[channel as usize];
                let patch = json!({
                    "program": program,
                    "bank": {"msb": c.bank_msb, "lsb": c.bank_lsb},
                });
                cx.state(self.state_channel(channel, patch));
            }
            Message::ChannelPressure { channel, pressure } => {
                cx.state(self.state_channel(channel, json!({"pressure": pressure})));
            }
            Message::PitchBend { channel, value } => {
                let patch = json!({"pitch_bend": value as i64 - 8192});
                cx.state(self.state_channel(channel, patch));
            }
            Message::SysEx(ref body) => {
                if let Some((address, args)) = self.sysex(cx, body) {
                    self.event(cx, address, args, source);
                    return;
                }
            }
            Message::QuarterFrame { piece, value } => {
                if let Some((tc, forward)) = self.mtc.quarter_frame(piece, value) {
                    let mut mtc = tc.json();
                    mtc["running"] = json!(true);
                    mtc["direction"] = json!(if forward { "forward" } else { "reverse" });
                    mtc["source"] = json!("quarter_frame");
                    cx.state(json!({"mtc": mtc}));
                    cx.set_timer(MTC_IDLE, MTC_IDLE_MS);
                }
            }
            Message::SongPosition(beats) => {
                cx.state(json!({"sequencer": {"song_position": beats}}))
            }
            Message::SongSelect(song) => cx.state(json!({"sequencer": {"song": song}})),
            Message::Start | Message::Continue => cx.state(json!({"sequencer": {"running": true}})),
            Message::Stop => cx.state(json!({"sequencer": {"running": false}})),
            _ => {}
        }
        let quiet = matches!(
            m,
            Message::Clock | Message::ActiveSensing | Message::QuarterFrame { .. }
        );
        if !quiet || self.realtime_events {
            self.event(cx, kind, args, source);
        }
    }

    fn control_change(&mut self, cx: &mut Cx, channel: u8, controller: u8, value: u8) {
        let mut patch = Map::new();
        patch.insert("cc".into(), json!({controller.to_string(): value}));
        let c = &mut self.channels[channel as usize];
        let s = &mut c.selection;
        match controller {
            0 => c.bank_msb = Some(value),
            32 => c.bank_lsb = Some(value),
            98..=101 => {
                let rpn = controller >= 100;
                if s.rpn != rpn {
                    *s = Selection {
                        rpn,
                        ..Selection::default()
                    };
                }
                if controller == 99 || controller == 101 {
                    s.msb = Some(value);
                } else {
                    s.lsb = Some(value);
                }
                s.data_msb = None;
            }
            6 | 38 => {
                if let (Some(msb), Some(lsb)) = (s.msb, s.lsb) {
                    // 127/127 is the null parameter: nothing is selected.
                    if !(msb == 127 && lsb == 127) {
                        let number = (msb as u16) << 7 | lsb as u16;
                        let system = if s.rpn { "rpn" } else { "nrpn" };
                        let entry = if controller == 6 {
                            s.data_msb = Some(value);
                            json!({"value_msb": value, "value": (value as u16) << 7})
                        } else {
                            let coarse = s.data_msb.unwrap_or(0);
                            json!({"value_lsb": value, "value": (coarse as u16) << 7 | value as u16})
                        };
                        patch.insert(system.into(), json!({number.to_string(): entry}));
                    }
                }
            }
            // All sound off and all notes off (and the mode messages, which
            // also turn notes off) end every note on the channel.
            120 | 123..=127 => {
                let mut notes = Map::new();
                for (note, on) in self.notes_on[channel as usize].iter_mut().enumerate() {
                    if *on {
                        *on = false;
                        notes.insert(note.to_string(), json!({"on": false}));
                    }
                }
                if !notes.is_empty() {
                    patch.insert("notes".into(), Value::Object(notes));
                }
            }
            _ => {}
        }
        cx.state(self.state_channel(channel, Value::Object(patch)));
    }

    /// A received SysEx: MTC, MMC or anything else. Returns the event for a
    /// universal message it understood.
    fn sysex(&mut self, cx: &mut Cx, body: &[u8]) -> Option<(&'static str, Value)> {
        let hex = midi::to_hex(&Message::SysEx(body.to_vec()).encode());
        cx.state(json!({"sysex": {"last": hex}}));
        let (device, sub, data) = midi::universal_real_time(body)?;
        match (sub, data) {
            (midi::SUB_MTC, [0x01, time @ ..]) => {
                let tc = Timecode::from_bytes(time)?;
                self.mtc.reset();
                cx.cancel_timer(MTC_IDLE);
                let mut mtc = tc.json();
                mtc["running"] = json!(false);
                mtc["source"] = json!("full_frame");
                cx.state(json!({"mtc": mtc}));
                let mut args = tc.json();
                args["device_id"] = json!(device);
                Some(("mtc_full_frame", args))
            }
            (midi::SUB_MTC, [0x02, groups @ ..]) if groups.len() >= 9 => {
                let bits: String = groups[..8]
                    .iter()
                    .rev()
                    .map(|n| format!("{:x}", n & 0x0F))
                    .collect();
                let flags = groups[8] & 0x03;
                cx.state(json!({"mtc": {"user_bits": bits, "user_bits_flags": flags}}));
                Some((
                    "mtc_user_bits",
                    json!({"device_id": device, "user_bits": bits, "flags": flags}),
                ))
            }
            (midi::SUB_MMC_COMMAND, data) => {
                let mut commands = Vec::new();
                let mut mmc = Map::new();
                for (code, args) in midi::mmc_items(data) {
                    let name = midi::mmc_name(code);
                    match code {
                        0x44 if args.first() == Some(&0x01) => {
                            if let Some(t) = midi::read_standard_time(&args[1..]) {
                                mmc.insert("locate".into(), t);
                            }
                        }
                        0x44 if args.first() == Some(&0x00) => {
                            if let Some(r) = args.get(1) {
                                mmc.insert(
                                    "locate".into(),
                                    json!({"register": mmc_field_name(*r)}),
                                );
                            }
                        }
                        0x45..=0x47 => {
                            if let Some(speed) = midi::mmc_speed_value(args) {
                                mmc.insert("speed".into(), json!(speed));
                            }
                        }
                        _ => {}
                    }
                    if matches!(code, 0x01..=0x0B | 0x45..=0x48) {
                        mmc.insert("transport".into(), json!(name));
                    }
                    commands.push(json!({"command": name, "hex": midi::to_hex(args)}));
                }
                if let Some(last) = commands.last() {
                    mmc.insert("command".into(), last["command"].clone());
                }
                mmc.insert("device_id".into(), json!(device));
                cx.state(json!({"mmc": mmc}));
                Some(("mmc", json!({"device_id": device, "commands": commands})))
            }
            (midi::SUB_MMC_RESPONSE, data) => {
                let mut fields = Map::new();
                for (code, value) in midi::mmc_responses(data) {
                    match code {
                        0x01..=0x1F => {
                            let mut full = [0u8; 5];
                            full.copy_from_slice(value);
                            self.mmc_times.insert((device, code), full);
                            fields.insert(mmc_field_name(code), mmc_field_value(code, value));
                        }
                        // A short time code updates the frames of the full
                        // field it shortens, once that has been heard.
                        0x20..=0x3F => {
                            let full = code - 0x20;
                            if let Some(time) = self.mmc_times.get_mut(&(device, full)) {
                                time[3] = value[0];
                                time[4] = value[1];
                                let time = *time;
                                fields.insert(mmc_field_name(full), mmc_field_value(full, &time));
                            }
                        }
                        _ => {
                            fields.insert(mmc_field_name(code), mmc_field_value(code, value));
                        }
                    }
                }
                cx.state(json!({"mmc": {"responses": {device.to_string(): fields.clone()}}}));
                Some((
                    "mmc_response",
                    json!({"device_id": device, "fields": fields}),
                ))
            }
            _ => None,
        }
    }
}

/// Whether `bytes` end on a whole message: no SysEx left open and no
/// channel or system common message short of its data.
fn ends_whole(bytes: &[u8]) -> bool {
    let mut open_sysex = false;
    let mut status: Option<u8> = None;
    let mut have = 0usize;
    let mut need = 0usize;
    for &b in bytes {
        match b {
            0xF8..=0xFF => {}
            0xF0 => {
                open_sysex = true;
                need = 0;
            }
            0xF7 => {
                open_sysex = false;
                need = 0;
            }
            0x80..=0xF6 => {
                open_sysex = false;
                status = Some(b);
                have = 0;
                need = match b {
                    0x80..=0xBF | 0xE0..=0xEF | 0xF2 => 2,
                    0xC0..=0xDF | 0xF1 | 0xF3 => 1,
                    _ => 0,
                };
            }
            _ if open_sysex => {}
            _ => {
                have += 1;
                if have == need {
                    have = 0;
                    // Running status repeats channel messages only.
                    if status.is_some_and(|s| s >= 0xF0) {
                        need = 0;
                    }
                }
            }
        }
    }
    !open_sysex && have == 0
}

impl Module for GenericMidi {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({"link": self.link.kind()}));
        cx.state(self.generator.state());
        self.link.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(result) = self.generator_command(cx, name, params) {
            cx.complete(id, result);
            return;
        }
        let result = self
            .messages(name, params)
            .and_then(|messages| self.link.send(cx, &messages))
            .map(|()| Outcome::Unverified);
        cx.complete(id, result);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        let messages = self.link.datagram(cx, socket, from, data);
        self.receive(cx, messages);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.link.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        let messages = self.link.tcp(cx, socket, input);
        self.receive(cx, messages);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == MTC_IDLE {
            cx.state(json!({"mtc": {"running": false}}));
            return;
        }
        if key == MTC_GENERATOR {
            self.generator_run(cx);
            return;
        }
        self.link.timer(cx, key);
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.link.stop(cx);
    }

    fn update_settings(&mut self, _cx: &mut Cx, _settings: &Params) -> SettingsUpdate {
        SettingsUpdate::Restart
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        self.link.reconnect(cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;
    use crate::midi::Rate;
    use crate::module::{Action, Connection};

    fn ctx(settings: Value) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        }
    }

    fn port_module() -> GenericMidi {
        let mut m = GenericMidi::new(ctx(
            json!({"link": "port", "midi_input": "In", "midi_output": "Out"}),
        ))
        .unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a, Action::MidiOpen { port, .. }
            if port.input.as_deref() == Some("In") && port.output.as_deref() == Some("Out"))));
        let mut cx = Cx::new(1);
        m.tcp(&mut cx, "midi-port", TcpInput::Connected);
        assert!(cx
            .take()
            .contains(&Action::Connection(Connection::Connected)));
        m
    }

    fn run(
        m: &mut GenericMidi,
        name: &str,
        params: Value,
    ) -> (Vec<u8>, Option<crate::module::CommandResult>) {
        let mut cx = Cx::new(10);
        m.command(&mut cx, 1, name, params.as_object().unwrap());
        let mut sent = Vec::new();
        let mut result = None;
        for a in cx.take() {
            match a {
                Action::TcpSend { data, .. } => sent.extend(data),
                Action::Complete { result: r, .. } => result = Some(r),
                _ => {}
            }
        }
        (sent, result)
    }

    fn wire(m: &mut GenericMidi, name: &str, params: Value) -> Vec<u8> {
        let (sent, result) = run(m, name, params);
        assert_eq!(result, Some(Ok(Outcome::Unverified)), "{name}");
        sent
    }

    /// What a step of the module sent, and when it asks to run again.
    fn step(actions: Vec<Action>) -> (Vec<u8>, Option<u64>, Option<Value>) {
        let mut sent = Vec::new();
        let mut next = None;
        let mut state = None;
        for a in actions {
            match a {
                Action::TcpSend { data, .. } => sent.extend(data),
                Action::SetTimer { key, after } if key == MTC_GENERATOR => next = Some(after),
                Action::State(patch) if patch.get("mtc_generator").is_some() => {
                    state = Some(patch["mtc_generator"].clone())
                }
                _ => {}
            }
        }
        (sent, next, state)
    }

    #[test]
    fn the_mtc_generator_locates_runs_and_stops() {
        let mut m = port_module();
        // Located while stopped: a full message to device 5, at 30df.
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "mtc_generator_locate",
            json!({"hours": 1, "minutes": 0, "seconds": 59, "frames": 28, "rate": "30df", "device_id": 5})
                .as_object()
                .unwrap(),
        );
        let (sent, next, state) = step(cx.take());
        assert_eq!(sent, [0xF0, 0x7F, 5, 0x01, 0x01, 0x41, 0, 59, 28, 0xF7]);
        assert_eq!(next, None);
        assert_eq!(state.unwrap()["timecode"], "01:00:59:28");

        // Started: quarter frames by the clock, 1001/120 ms apart at 29.97.
        let mut t = 1000;
        let mut cx = Cx::new(t);
        m.command(&mut cx, 2, "mtc_generator_start", &Params::new());
        let (mut wire, mut next, state) = step(cx.take());
        assert_eq!(state.unwrap()["running"], true);
        let mut times = vec![t];
        while wire.len() < 32 {
            t += next.expect("the generator runs on");
            let mut cx = Cx::new(t);
            m.timer(&mut cx, MTC_GENERATOR);
            let (sent, n, _) = step(cx.take());
            if !sent.is_empty() {
                times.push(t);
            }
            wire.extend(sent);
            next = n;
        }
        let pieces: Vec<(u8, u8)> = wire.chunks(2).map(|c| (c[1] >> 4, c[1] & 0x0F)).collect();
        assert!(wire.chunks(2).all(|c| c[0] == 0xF1));
        // 01:00:59:28, then two frames on across the minute, where
        // drop-frame skips frames 0 and 1: 01:01:00:02. Rate 2 (30df) in
        // piece 7.
        let values: Vec<u8> = pieces.iter().map(|p| p.1).collect();
        assert_eq!(values, [0xC, 1, 0xB, 3, 0, 0, 1, 4, 2, 0, 0, 0, 1, 0, 1, 4]);
        assert_eq!(
            pieces.iter().map(|p| p.0).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7]
        );
        // The sixteenth went out 125 ms after the first (15 x 1001/120).
        assert_eq!(*times.last().unwrap(), 1125);

        // Stopped at the frame reached.
        let mut cx = Cx::new(t);
        m.command(&mut cx, 3, "mtc_generator_stop", &Params::new());
        let (sent, _, state) = step(cx.take());
        assert!(sent.is_empty());
        let state = state.unwrap();
        assert_eq!(state["running"], false);
        assert_eq!(state["timecode"], "01:01:00:04");

        // Started again and held up for a second: whole runs are skipped,
        // never more than eight sent at once, and the time follows the
        // clock.
        let mut cx = Cx::new(10_000);
        m.command(&mut cx, 4, "mtc_generator_start", &Params::new());
        cx.take();
        let mut cx = Cx::new(11_000);
        m.timer(&mut cx, MTC_GENERATOR);
        let (sent, _, _) = step(cx.take());
        assert!(!sent.is_empty() && sent.len() <= 16);
        let frame = m.generator.frame();
        // About 30 frames on from 01:01:00:04.
        assert_eq!((frame.minutes, frame.seconds), (1, 1));
    }

    #[test]
    fn commands_put_their_messages_on_the_wire() {
        let mut m = port_module();
        assert_eq!(
            wire(
                &mut m,
                "note_on",
                json!({"channel": 2, "note": 60, "velocity": 100})
            ),
            [0x91, 60, 100]
        );
        assert_eq!(
            wire(
                &mut m,
                "note_off",
                json!({"channel": 16, "note": 60, "velocity": 64})
            ),
            [0x8F, 60, 64]
        );
        assert_eq!(
            wire(
                &mut m,
                "control_change_14bit",
                json!({"channel": 1, "controller": 7, "value": 16383})
            ),
            [0xB0, 7, 127, 0xB0, 39, 127]
        );
        assert_eq!(
            wire(
                &mut m,
                "program_change",
                json!({"channel": 1, "program": 5, "bank_msb": 1, "bank_lsb": 2})
            ),
            [0xB0, 0, 1, 0xB0, 32, 2, 0xC0, 5]
        );
        assert_eq!(
            wire(&mut m, "pitch_bend", json!({"channel": 1, "value": -8192})),
            [0xE0, 0, 0]
        );
        assert_eq!(
            wire(&mut m, "pitch_bend", json!({"channel": 1, "value": 0})),
            [0xE0, 0, 0x40]
        );
        assert_eq!(
            wire(
                &mut m,
                "nrpn",
                json!({"channel": 3, "parameter": 300, "value_msb": 10, "value_lsb": 5, "deselect": true})
            ),
            [0xB2, 99, 2, 0xB2, 98, 44, 0xB2, 6, 10, 0xB2, 38, 5, 0xB2, 101, 127, 0xB2, 100, 127]
        );
        assert_eq!(
            wire(
                &mut m,
                "rpn",
                json!({"channel": 1, "parameter": 0, "value_msb": 2})
            ),
            [0xB0, 101, 0, 0xB0, 100, 0, 0xB0, 6, 2]
        );
        assert_eq!(
            wire(
                &mut m,
                "channel_mode",
                json!({"channel": 1, "mode": "mono_on", "channels": 4})
            ),
            [0xB0, 126, 4]
        );
        assert_eq!(
            wire(&mut m, "sysex", json!({"hex": "F0 43 10 4C F7"})),
            [0xF0, 0x43, 0x10, 0x4C, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "sysex", json!({"hex": "7E 7F 06 01"})),
            [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "raw", json!({"hex": "90 3C 40 3D 40"})),
            [0x90, 0x3C, 0x40, 0x90, 0x3D, 0x40]
        );
        assert_eq!(
            wire(&mut m, "song_position", json!({"beats": 200})),
            [0xF2, 0x48, 0x01]
        );
        assert_eq!(
            wire(&mut m, "realtime", json!({"message": "start"})),
            [0xFA]
        );
        assert_eq!(
            wire(
                &mut m,
                "mtc_full_frame",
                json!({"hours": 1, "minutes": 2, "seconds": 3, "frames": 4, "rate": "25", "device_id": 127})
            ),
            [0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x21, 2, 3, 4, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mtc_quarter_frame", json!({"piece": 7, "value": 6})),
            [0xF1, 0x76]
        );
        assert_eq!(
            wire(
                &mut m,
                "mtc_user_bits",
                json!({"user_bits": "12345678", "flags": 1})
            ),
            [0xF0, 0x7F, 0x7F, 0x01, 0x02, 8, 7, 6, 5, 4, 3, 2, 1, 1, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mmc", json!({"command": "play", "device_id": 1})),
            [0xF0, 0x7F, 0x01, 0x06, 0x02, 0xF7]
        );
        assert_eq!(
            wire(
                &mut m,
                "mmc_locate",
                json!({"hours": 1, "minutes": 0, "seconds": 0, "frames": 0, "rate": "30"})
            ),
            [0xF0, 0x7F, 0x7F, 0x06, 0x44, 0x06, 0x01, 0x61, 0, 0, 0, 0, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mmc_locate_register", json!({"register": 0})),
            [0xF0, 0x7F, 0x7F, 0x06, 0x44, 0x02, 0x00, 0x08, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mmc_shuttle", json!({"speed": -2.0})),
            [0xF0, 0x7F, 0x7F, 0x06, 0x47, 0x03, 0x42, 0x00, 0x00, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mmc_step", json!({"steps": -3})),
            [0xF0, 0x7F, 0x7F, 0x06, 0x48, 0x01, 0x43, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "mmc_read", json!({"field": "motion_control_tally"})),
            [0xF0, 0x7F, 0x7F, 0x06, 0x42, 0x01, 0x48, 0xF7]
        );
        assert_eq!(
            wire(
                &mut m,
                "mmc_write_time",
                json!({"field": "gp1", "hours": 0, "minutes": 1, "seconds": 0, "frames": 2, "rate": "30df", "subframes": 50})
            ),
            [0xF0, 0x7F, 0x7F, 0x06, 0x40, 0x06, 0x09, 0x40, 1, 0, 2, 50, 0xF7]
        );
        // Refusals.
        let (_, r) = run(&mut m, "raw", json!({"hex": "90 3C"}));
        assert!(matches!(r, Some(Err(CommandError::InvalidParams { .. }))));
        let (_, r) = run(&mut m, "raw", json!({"hex": "F0 01 02"}));
        assert!(matches!(r, Some(Err(CommandError::InvalidParams { .. }))));
        let (_, r) = run(&mut m, "sysex", json!({"hex": "01 90 02"}));
        assert!(matches!(r, Some(Err(CommandError::InvalidParams { .. }))));
        let (_, r) = run(
            &mut m,
            "mtc_full_frame",
            json!({"hours": 0, "minutes": 1, "seconds": 0, "frames": 0, "rate": "30df"}),
        );
        assert!(matches!(r, Some(Err(CommandError::InvalidParams { .. }))));
    }

    #[test]
    fn a_closed_port_refuses_commands_and_is_opened_again() {
        let mut m = port_module();
        let mut cx = Cx::new(5);
        m.tcp(
            &mut cx,
            "midi-port",
            TcpInput::Closed {
                reason: "midi: gone".into(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: "midi-port-retry",
            after: 1000
        }));
        let (_, r) = run(
            &mut m,
            "note_on",
            json!({"channel": 1, "note": 1, "velocity": 1}),
        );
        assert_eq!(r, Some(Err(CommandError::NotConnected)));
        let mut cx = Cx::new(1005);
        m.timer(&mut cx, "midi-port-retry");
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::MidiOpen { .. })));
        // An input-only port has nothing to send to.
        let mut m = GenericMidi::new(ctx(json!({"link": "port", "midi_input": "In"}))).unwrap();
        let (_, r) = run(
            &mut m,
            "note_on",
            json!({"channel": 1, "note": 1, "velocity": 1}),
        );
        assert!(matches!(r, Some(Err(CommandError::InvalidParams { .. }))));
        assert!(GenericMidi::new(ctx(json!({"link": "port"}))).is_err());
        assert!(
            GenericMidi::new(ctx(json!({"link": "rtp"}))).is_err(),
            "rtp needs a host"
        );
    }

    fn receive(m: &mut GenericMidi, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(100);
        m.tcp(&mut cx, "midi-port", TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn merged(actions: &[Action]) -> Value {
        let mut state = json!({});
        for a in actions {
            if let Action::State(patch) = a {
                json_patch::merge(&mut state, patch);
            }
        }
        state
    }

    mod json_patch {
        use serde_json::Value;
        pub(super) fn merge(target: &mut Value, patch: &Value) {
            if let Value::Object(p) = patch {
                if !target.is_object() {
                    *target = Value::Object(Default::default());
                }
                let t = target.as_object_mut().unwrap();
                for (k, v) in p {
                    if v.is_null() {
                        t.remove(k);
                    } else {
                        merge(t.entry(k.clone()).or_insert(Value::Null), v);
                    }
                }
            } else {
                *target = patch.clone();
            }
        }
    }

    fn events(actions: &[Action]) -> Vec<(String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Message { address, args, .. } => Some((address.clone(), args.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn received_messages_are_kept_and_reported() {
        let mut m = port_module();
        let a = receive(
            &mut m,
            &[
                0x90, 60, 100, // note on
                0xB0, 7, 99, // volume
                0xB0, 0, 1, 0xB0, 32, 2, 0xC0, 9, // bank and program
                0xB0, 99, 1, 0xB0, 98, 2, 0xB0, 6, 3, 0xB0, 38, 4, // NRPN 130 = 3/4
                0xE0, 0, 0x40, // pitch bend centre
                0xF8, 0xFE, // clock, active sensing: not reported
                0xFA,
            ],
        );
        let s = merged(&a);
        assert_eq!(
            s["channels"]["1"]["notes"]["60"],
            json!({"on": true, "velocity": 100})
        );
        assert_eq!(s["channels"]["1"]["cc"]["7"], 99);
        assert_eq!(s["channels"]["1"]["program"], 9);
        assert_eq!(s["channels"]["1"]["bank"], json!({"msb": 1, "lsb": 2}));
        assert_eq!(
            s["channels"]["1"]["nrpn"]["130"],
            json!({"value_msb": 3, "value_lsb": 4, "value": 388})
        );
        assert_eq!(s["channels"]["1"]["pitch_bend"], 0);
        assert_eq!(s["sequencer"]["running"], true);
        assert_eq!(s["received_count"], 13);
        let e = events(&a);
        assert_eq!(
            e[0],
            (
                "note_on".into(),
                json!({"channel": 1, "note": 60, "velocity": 100})
            )
        );
        assert!(!e.iter().any(|(k, _)| k == "clock" || k == "active_sensing"));
        assert_eq!(e.last().unwrap().0, "start");
        // All notes off ends the note.
        let s = merged(&receive(&mut m, &[0xB0, 123, 0]));
        assert_eq!(s["channels"]["1"]["notes"]["60"]["on"], false);
    }

    #[test]
    fn mtc_and_mmc_are_kept() {
        let mut m = port_module();
        let tc = Timecode::new(10, 0, 0, 0, Rate::Fps25).unwrap();
        let qf: Vec<u8> = midi::quarter_frames(&tc)
            .iter()
            .flat_map(Message::encode)
            .collect();
        let mut bytes = qf.clone();
        bytes.extend_from_slice(&qf);
        let a = receive(&mut m, &bytes);
        let s = merged(&a);
        assert_eq!(s["mtc"]["timecode"], "10:00:00:02");
        assert_eq!(s["mtc"]["rate"], "25");
        assert_eq!(s["mtc"]["running"], true);
        assert!(a.contains(&Action::SetTimer {
            key: MTC_IDLE,
            after: MTC_IDLE_MS
        }));
        assert!(events(&a).is_empty(), "quarter frames are quiet by default");
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, MTC_IDLE);
        assert_eq!(merged(&cx.take())["mtc"]["running"], false);
        // Full frame.
        let a = receive(&mut m, &[0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x61, 2, 3, 4, 0xF7]);
        let s = merged(&a);
        assert_eq!(s["mtc"]["timecode"], "01:02:03:04");
        assert_eq!(s["mtc"]["source"], "full_frame");
        assert_eq!(events(&a)[0].0, "mtc_full_frame");
        // An MMC locate then play from device 2.
        let a = receive(
            &mut m,
            &[
                0xF0, 0x7F, 0x02, 0x06, 0x44, 0x06, 0x01, 0x21, 0, 0, 0, 0, 0x02, 0xF7,
            ],
        );
        let s = merged(&a);
        assert_eq!(s["mmc"]["command"], "play");
        assert_eq!(s["mmc"]["transport"], "play");
        assert_eq!(s["mmc"]["locate"]["timecode"], "01:00:00:00");
        assert_eq!(s["mmc"]["device_id"], 2);
        // An MMC response: motion control tally and selected time code.
        let a = receive(
            &mut m,
            &[
                0xF0, 0x7F, 0x05, 0x07, 0x48, 0x03, 0x02, 0x7F, 0x01, 0x01, 0x61, 1, 2, 0x23, 0x00,
                0xF7,
            ],
        );
        let s = merged(&a);
        let fields = &s["mmc"]["responses"]["5"];
        assert_eq!(fields["motion_control_tally"]["state"], "play");
        assert_eq!(fields["motion_control_tally"]["process"], "none");
        assert_eq!(fields["selected_time_code"]["timecode"], "01:01:02:03");
        assert_eq!(fields["selected_time_code"]["status"]["estimated"], false);
    }
}
