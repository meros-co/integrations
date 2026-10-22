//! d&b audiotechnik amplifiers over AES70 (OCP.1): the AES70 module, with
//! the amplifiers' objects named and their controls as commands.
//!
//! d&b publishes its amplifiers' AES70 objects in its own integration
//! plug-ins: the Q-SYS plug-in (a Lua script under a BSD-style licence) lists
//! the object numbers of each amplifier family, and the AMX module's
//! interface specification gives the OCP.1 port and an example PDU. The
//! object numbers are built from four fields, `type << 28 | record << 20 |
//! channel << 15 | object`, where the channel is the output channel (A to D
//! as 1 to 4) and the record an input or AmpPreset number. There are three
//! families of numbers: the D80, D20, 30D and 10D; the D90, D40, 40D, D25 and
//! 25D; and the 5D and 5DM.
//!
//! The named objects are handed to the AES70 module, which asks for their
//! classes on connecting and, when the device is monitored, reads them and
//! subscribes to their changes, keyed in state by name ("A/Mute",
//! "Power"). Each named command becomes one of the AES70 module's commands
//! on the object's name; AmpPresets, identification and the clock are
//! methods called with `call_method`. Everything else (the session, keep-
//! alives, meters, commands-only opening) is the AES70 module's.
//!
//! d&b publishes no object list for the DS10 and DS100: they are reached
//! with the generic AES70 commands, by walking their object tree.

use serde_json::{json, Value};

use super::aes70::Aes70;
use crate::catalog::Params;
use crate::module::{CommandError, CommandId, Cx, Key, Module, OpenContext, TcpInput, WsInput};

/// The amplifier families, by how d&b numbers their objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// D80, D20, 30D, 10D.
    Dx,
    /// D90, D40, 40D, D25, 25D.
    Dy,
    /// 5D, 5DM.
    Five,
    /// DS10, DS100: no published object list.
    None,
}

/// The family and the input-mode switch a model has.
pub(crate) fn family(model: &str) -> Family {
    match model {
        "d80" | "d20" | "30d" | "10d" => Family::Dx,
        "d90" | "d40" | "40d" | "d25" | "25d" => Family::Dy,
        "5d" | "5dm" => Family::Five,
        _ => Family::None,
    }
}

/// The OCP.1 TCP port d&b's integrations use for a model: 30013 on the D80,
/// D20, 30D and 10D, 50014 on the others; none known for the DS10 and DS100.
pub(crate) fn default_port(model: &str) -> Option<u16> {
    match family(model) {
        Family::Dx => Some(30013),
        Family::Dy | Family::Five => Some(50014),
        Family::None => None,
    }
}

/// An object's number from d&b's four fields.
fn ono(record: u32, channel: u32, object: u32) -> u32 {
    (1 << 28) | (record << 20) | (channel << 15) | object
}

/// The objects each family numbers, as (name, object field), by the names in
/// d&b's Q-SYS plug-in (its object names in comments). Zero is absent.
struct Table {
    power_ok: u32,
    general_error: u32,
    device_error: u32,
    device_error_text: u32,
    channel_error: u32,
    channel_error_text: u32,
    isp: u32,
    gr: u32,
    ovl: u32,
    headroom: u32,
    selected_preset: u32,
    preset_modified: u32,
    power: u32,
    device_name: u32,
    wink: u32,
    channel_name: u32,
    mute: u32,
    level: u32,
    delay: u32,
    delay_on: u32,
    load_monitoring: u32,
    filter: [u32; 3],
    input_monitoring_analog: u32,
    input_monitoring_digital: u32,
    input_monitoring_error_analog: u32,
    input_monitoring_error_digital: u32,
    fallback_mode: u32,
    fallback_active: u32,
    override_mode: u32,
    override_active: u32,
    input_gain_analog: u32,
    input_gain_digital: u32,
    input_gain_enable: u32,
    preset_name: u32,
    preset_state: u32,
    time_manager: u32,
    input_enable: [u32; 8],
    input_modes: u32,
}

const DX: Table = Table {
    power_ok: 0x300,
    general_error: 0x500,
    device_error: 0x515,
    device_error_text: 0x502,
    channel_error: 0x600,
    channel_error_text: 0x602,
    isp: 0x400,
    gr: 0x41D,
    ovl: 0x41E,
    headroom: 0x40C,
    selected_preset: 0x120,
    preset_modified: 0x314,
    power: 0x100,
    device_name: 0x10D,
    wink: 0x327,
    channel_name: 0x215,
    mute: 0x205,
    level: 0x206,
    delay: 0x415,
    delay_on: 0x203,
    load_monitoring: 0x900,
    filter: [0x200, 0x201, 0x202],
    input_monitoring_analog: 0xF00,
    input_monitoring_digital: 0xF0A,
    input_monitoring_error_analog: 0xF09,
    input_monitoring_error_digital: 0xF13,
    fallback_mode: 0x132,
    fallback_active: 0x35A,
    override_mode: 0x133,
    override_active: 0x35B,
    input_gain_analog: 0xE03,
    input_gain_digital: 0xE07,
    input_gain_enable: 0x131,
    preset_name: 0x1000,
    preset_state: 0x1001,
    time_manager: 0x09,
    input_enable: [0x220, 0x221, 0x222, 0x223, 0x224, 0x225, 0x226, 0x227],
    input_modes: 0x10F,
};

const DY: Table = Table {
    power_ok: 0x300,
    general_error: 0x500,
    device_error: 0x514,
    device_error_text: 0x502,
    channel_error: 0x600,
    channel_error_text: 0x602,
    isp: 0x400,
    gr: 0x401,
    ovl: 0x402,
    headroom: 0x40A,
    selected_preset: 0x114,
    preset_modified: 0x311,
    power: 0x100,
    device_name: 0x10C,
    wink: 0x318,
    channel_name: 0x213,
    mute: 0x205,
    level: 0x206,
    delay: 0x412,
    delay_on: 0x203,
    load_monitoring: 0x900,
    filter: [0x200, 0x201, 0x202],
    input_monitoring_analog: 0x1100,
    input_monitoring_digital: 0x1200,
    input_monitoring_error_analog: 0x1109,
    input_monitoring_error_digital: 0x1209,
    fallback_mode: 0x124,
    fallback_active: 0x338,
    override_mode: 0x125,
    override_active: 0x339,
    input_gain_analog: 0xE03,
    input_gain_digital: 0xF03,
    input_gain_enable: 0x123,
    preset_name: 0x1400,
    preset_state: 0x1401,
    time_manager: 0x0A,
    input_enable: [0x21E, 0x21F, 0x220, 0x221, 0x222, 0x223, 0x224, 0x225],
    input_modes: 0,
};

const FIVE: Table = Table {
    power_ok: 0x301,
    general_error: 0x501,
    device_error: 0x508,
    device_error_text: 0x503,
    channel_error: 0x601,
    channel_error_text: 0x603,
    isp: 0x401,
    gr: 0x402,
    ovl: 0x403,
    headroom: 0x404,
    selected_preset: 0,
    preset_modified: 0,
    power: 0x101,
    device_name: 0x108,
    wink: 0x302,
    channel_name: 0x204,
    mute: 0x205,
    level: 0x206,
    delay: 0x40A,
    delay_on: 0x207,
    load_monitoring: 0x901,
    filter: [0x208, 0x209, 0x20A],
    input_monitoring_analog: 0xE01,
    input_monitoring_digital: 0xF01,
    input_monitoring_error_analog: 0xE0A,
    input_monitoring_error_digital: 0xF0A,
    fallback_mode: 0x10A,
    fallback_active: 0x307,
    override_mode: 0x10C,
    override_active: 0x308,
    input_gain_analog: 0xC02,
    input_gain_digital: 0xD02,
    input_gain_enable: 0x109,
    preset_name: 0,
    preset_state: 0,
    time_manager: 0x0A,
    input_enable: [0x210, 0x211, 0x212, 0x213, 0x214, 0x215, 0x216, 0x217],
    input_modes: 0,
};

/// The AmpPreset agent (CdbOcaAmpPreset), the same on every family that
/// has AmpPresets.
const AMP_PRESET_AGENT: u32 = 10003;
/// AmpPresets, numbered from 1.
const PRESETS: u32 = 15;
/// Output channels A to D.
const CHANNELS: [&str; 4] = ["A", "B", "C", "D"];
/// Inputs in the order of the input-routing switches (InputEnable1 to 8).
const INPUTS: [&str; 8] = ["A1", "A2", "A3", "A4", "D1", "D2", "D3", "D4"];

fn table(family: Family) -> Option<&'static Table> {
    match family {
        Family::Dx => Some(&DX),
        Family::Dy => Some(&DY),
        Family::Five => Some(&FIVE),
        Family::None => None,
    }
}

/// Whether a model has the D80 and D20's input-mode switch.
fn has_input_modes(model: &str) -> bool {
    matches!(model, "d80" | "d20")
}

/// Every object a model's family numbers, by name.
pub(crate) fn named_objects(model: &str) -> Vec<(String, u32)> {
    let Some(t) = table(family(model)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, u32)> = Vec::new();
    let mut add = |name: String, record: u32, channel: u32, object: u32| {
        if object != 0 {
            out.push((name, ono(record, channel, object)));
        }
    };
    add("Power".into(), 0, 0, t.power);
    add("PowerOk".into(), 0, 0, t.power_ok);
    add("GeneralError".into(), 0, 0, t.general_error);
    add("DeviceError".into(), 0, 0, t.device_error);
    add("DeviceErrorText".into(), 0, 0, t.device_error_text);
    add("DeviceName".into(), 0, 0, t.device_name);
    add("Wink".into(), 0, 0, t.wink);
    add("AmpPreset/Selected".into(), 0, 0, t.selected_preset);
    add("AmpPreset/Modified".into(), 0, 0, t.preset_modified);
    for n in 1..=PRESETS {
        add(format!("AmpPreset {n}/Name"), n, 0, t.preset_name);
        add(format!("AmpPreset {n}/Stored"), n, 0, t.preset_state);
    }
    add("InputFallback/Mode".into(), 0, 0, t.fallback_mode);
    add("InputFallback/Active".into(), 0, 0, t.fallback_active);
    add("InputOverride/Mode".into(), 0, 0, t.override_mode);
    add("InputOverride/Active".into(), 0, 0, t.override_active);
    add("InputGain/Enabled".into(), 0, 0, t.input_gain_enable);
    if has_input_modes(model) {
        add("InputMode".into(), 0, 0, t.input_modes);
    }
    for (i, c) in CHANNELS.iter().enumerate() {
        let ch = i as u32 + 1;
        add(format!("{c}/Name"), 0, ch, t.channel_name);
        add(format!("{c}/Mute"), 0, ch, t.mute);
        add(format!("{c}/Level"), 0, ch, t.level);
        add(format!("{c}/Delay"), 0, ch, t.delay);
        add(format!("{c}/DelayOn"), 0, ch, t.delay_on);
        add(format!("{c}/LoadMonitoring"), 0, ch, t.load_monitoring);
        for (f, object) in t.filter.iter().enumerate() {
            add(format!("{c}/Filter{}", f + 1), 0, ch, *object);
        }
        add(format!("{c}/Error"), 0, ch, t.channel_error);
        add(format!("{c}/ErrorText"), 0, ch, t.channel_error_text);
        add(format!("{c}/ISP"), 0, ch, t.isp);
        add(format!("{c}/GR"), 0, ch, t.gr);
        add(format!("{c}/OVL"), 0, ch, t.ovl);
        add(format!("{c}/Headroom"), 0, ch, t.headroom);
        for (input, object) in INPUTS.iter().zip(t.input_enable) {
            add(format!("{c}/Input {input}"), 0, ch, object);
        }
    }
    for i in 1..=4u32 {
        add(format!("Input A{i}/Gain"), i, 0, t.input_gain_analog);
        add(format!("Input D{i}/Gain"), i, 0, t.input_gain_digital);
        add(
            format!("Input A{i}/Monitoring"),
            i,
            0,
            t.input_monitoring_analog,
        );
        add(
            format!("Input D{i}/Monitoring"),
            i,
            0,
            t.input_monitoring_digital,
        );
        add(
            format!("Input A{i}/MonitoringError"),
            i,
            0,
            t.input_monitoring_error_analog,
        );
        add(
            format!("Input D{i}/MonitoringError"),
            i,
            0,
            t.input_monitoring_error_digital,
        );
    }
    out
}

pub(crate) struct DbAudio {
    inner: Aes70,
    model: String,
}

impl DbAudio {
    pub(crate) fn new(ctx: OpenContext, port: u16) -> DbAudio {
        let model = ctx.model.clone();
        DbAudio {
            inner: Aes70::with_named(ctx, port, named_objects(&model)),
            model,
        }
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn text<'a>(params: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn number(params: &Params, key: &str) -> Result<f64, CommandError> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn integer(params: &Params, key: &str) -> Result<i64, CommandError> {
    params
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn flag(params: &Params, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(true)
}

fn channel(params: &Params) -> Result<&str, CommandError> {
    let c = text(params, "channel")?;
    CHANNELS
        .iter()
        .find(|x| **x == c)
        .copied()
        .ok_or_else(|| invalid(format!("unknown channel '{c}'")))
}

fn input(params: &Params) -> Result<&str, CommandError> {
    let i = text(params, "input")?;
    INPUTS
        .iter()
        .find(|x| **x == i)
        .copied()
        .ok_or_else(|| invalid(format!("unknown input '{i}'")))
}

fn preset(params: &Params, key: &str) -> Result<u8, CommandError> {
    let n = integer(params, key)?;
    u8::try_from(n)
        .ok()
        .filter(|n| (1..=PRESETS as u8).contains(n))
        .ok_or_else(|| invalid(format!("'{key}' must be 1 to {PRESETS}")))
}

/// An AES70 command on a named object.
fn on(
    name: impl Into<String>,
    command: &'static str,
    key: &str,
    value: Value,
) -> (&'static str, Params) {
    let mut p = Params::new();
    p.insert("object".into(), json!(name.into()));
    p.insert(key.into(), value);
    (command, p)
}

fn switch(name: impl Into<String>, position: i64) -> (&'static str, Params) {
    on(name, "set_switch", "position", json!(position))
}

/// A method called on an object by number, with its parameters as OCP.1
/// bytes.
fn method(ono: u32, method: &str, count: u8, params: &[u8]) -> (&'static str, Params) {
    let mut p = Params::new();
    p.insert("ono".into(), json!(ono));
    p.insert("method".into(), json!(method));
    p.insert("param_count".into(), json!(count));
    let hex: String = params.iter().map(|b| format!("{b:02x}")).collect();
    p.insert("params_hex".into(), json!(hex));
    ("call_method", p)
}

fn mode_position(params: &Params) -> Result<i64, CommandError> {
    match text(params, "mode")? {
        "off" => Ok(0),
        "manual" => Ok(1),
        "auto" => Ok(2),
        other => Err(invalid(format!("unknown mode '{other}'"))),
    }
}

/// The AES70 command a named command becomes, or None for one the AES70
/// module takes as it is.
fn translate(
    model: &str,
    name: &str,
    params: &Params,
    unix_millis: u64,
) -> Result<Option<(&'static str, Params)>, CommandError> {
    let fam = family(model);
    let t = table(fam);
    let needs_table = || {
        t.ok_or_else(|| CommandError::UnsupportedForModel {
            command: name.to_string(),
            model: model.to_string(),
        })
    };
    Ok(Some(match name {
        "set_power" => switch("Power", flag(params, "powered") as i64),
        "set_channel_mute" => on(
            format!("{}/Mute", channel(params)?),
            "set_mute",
            "muted",
            json!(flag(params, "muted")),
        ),
        "set_channel_level" => on(
            format!("{}/Level", channel(params)?),
            "set_gain",
            "gain_db",
            json!(number(params, "gain_db")?),
        ),
        "set_channel_delay" => on(
            format!("{}/Delay", channel(params)?),
            "set_delay",
            "delay_s",
            json!(number(params, "delay_ms")? / 1000.0),
        ),
        "set_channel_delay_enabled" => switch(
            format!("{}/DelayOn", channel(params)?),
            flag(params, "enabled") as i64,
        ),
        "set_channel_name" => on(
            format!("{}/Name", channel(params)?),
            "set_string",
            "value",
            json!(text(params, "name")?),
        ),
        "set_load_monitoring" => switch(
            format!("{}/LoadMonitoring", channel(params)?),
            flag(params, "enabled") as i64,
        ),
        "set_channel_filter" => {
            let c = channel(params)?;
            let value = integer(params, "value")?;
            match integer(params, "filter")? {
                f @ (1 | 2) => switch(format!("{c}/Filter{f}"), value),
                3 => on(format!("{c}/Filter3"), "set_int", "value", json!(value)),
                other => return Err(invalid(format!("unknown filter {other}"))),
            }
        }
        "set_input_routing" => switch(
            format!("{}/Input {}", channel(params)?, input(params)?),
            flag(params, "enabled") as i64,
        ),
        "set_input_gain" => on(
            format!("Input {}/Gain", input(params)?),
            "set_gain",
            "gain_db",
            json!(number(params, "gain_db")?),
        ),
        "set_input_gain_enabled" => switch("InputGain/Enabled", flag(params, "enabled") as i64),
        "set_input_monitoring" => switch(
            format!("Input {}/Monitoring", input(params)?),
            flag(params, "enabled") as i64,
        ),
        "set_input_fallback" => switch("InputFallback/Mode", mode_position(params)?),
        "set_input_fallback_active" => {
            switch("InputFallback/Active", flag(params, "active") as i64)
        }
        "set_input_override" => switch("InputOverride/Mode", mode_position(params)?),
        "set_input_override_active" => {
            switch("InputOverride/Active", flag(params, "active") as i64)
        }
        "set_input_mode" => {
            if !has_input_modes(model) {
                return Err(CommandError::UnsupportedForModel {
                    command: name.to_string(),
                    model: model.to_string(),
                });
            }
            switch("InputMode", integer(params, "mode")?)
        }
        "set_device_name" => on(
            "DeviceName",
            "set_string",
            "value",
            json!(text(params, "name")?),
        ),
        "identify" => {
            // d&b's wink: its own method 5.2 with a 16-bit value, 0x0200 to
            // start and 0 to stop, as d&b's plug-in sends it. A model with
            // no published objects identifies as any AES70 device does.
            let Some(t) = t else {
                return Ok(None);
            };
            let value: u16 = if flag(params, "enabled") { 0x0200 } else { 0 };
            method(ono(0, 0, t.wink), "5.2", 1, &value.to_be_bytes())
        }
        "load_amp_preset" | "backup_and_load_amp_preset" | "set_amp_preset_name" => {
            let t = needs_table()?;
            if t.preset_name == 0 {
                return Err(CommandError::UnsupportedForModel {
                    command: name.to_string(),
                    model: model.to_string(),
                });
            }
            let n = preset(params, "preset")?;
            match name {
                // CdbOcaAmpPreset 3.3: load an AmpPreset.
                "load_amp_preset" => method(AMP_PRESET_AGENT, "3.3", 1, &[n]),
                // 3.1: store the current settings in a backup slot (13 to
                // 15), then load the AmpPreset.
                "backup_and_load_amp_preset" => {
                    let backup = preset(params, "backup")?;
                    if !(13..=15).contains(&backup) {
                        return Err(invalid("'backup' must be 13, 14 or 15"));
                    }
                    method(AMP_PRESET_AGENT, "3.1", 2, &[n, backup])
                }
                _ => on(
                    format!("AmpPreset {n}/Name"),
                    "set_string",
                    "value",
                    json!(text(params, "name")?),
                ),
            }
        }
        "set_device_time" => {
            // OcaDeviceTimeManager 3.2 with an NTP time: seconds since 1900
            // and a fraction, as d&b's plug-in sets it.
            let t = needs_table()?;
            let unix = match params.get("unix_seconds").and_then(Value::as_i64) {
                Some(s) => s,
                None => (unix_millis / 1000) as i64,
            };
            let ntp = u32::try_from(unix + 2_208_988_800)
                .map_err(|_| invalid("'unix_seconds' is out of the NTP era"))?;
            let mut bytes = ntp.to_be_bytes().to_vec();
            bytes.extend(0u32.to_be_bytes());
            method(t.time_manager, "3.2", 1, &bytes)
        }
        _ => return Ok(None),
    }))
}

impl Module for DbAudio {
    fn start(&mut self, cx: &mut Cx) {
        self.inner.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match translate(&self.model, name, params, cx.unix_millis()) {
            Ok(Some((aes70, p))) => self.inner.command(cx, id, aes70, &p),
            Ok(None) => self.inner.command(cx, id, name, params),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.inner.tcp(cx, socket, input);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: std::net::SocketAddr, data: &[u8]) {
        self.inner.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.inner.socket_error(cx, socket, message);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.inner.ws(cx, socket, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        self.inner.timer(cx, key);
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.inner.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn t(model: &str, name: &str, p: Value) -> (&'static str, Params) {
        translate(model, name, &params(p), 0).unwrap().unwrap()
    }

    #[test]
    fn object_numbers_follow_the_documented_layout() {
        // The AMX module's example: channel A's mute on a D80 is 0x10008205.
        let named = named_objects("d80");
        let find = |name: &str| named.iter().find(|(n, _)| n == name).map(|(_, o)| *o);
        assert_eq!(find("A/Mute"), Some(0x1000_8205));
        assert_eq!(find("D/Level"), Some(0x1002_0206));
        assert_eq!(find("Input A3/Gain"), Some(0x1030_0E03));
        assert_eq!(find("AmpPreset 15/Name"), Some(0x10F0_1000));
        assert_eq!(find("InputMode"), Some(0x1000_010F));
        // Families differ; the 5D has no AmpPresets, the D40 no input modes.
        let d40 = named_objects("d40");
        assert!(d40.iter().any(|(n, o)| n == "Wink" && *o == 0x1000_0318));
        assert!(!d40.iter().any(|(n, _)| n == "InputMode"));
        let five = named_objects("5d");
        assert!(!five.iter().any(|(n, _)| n.starts_with("AmpPreset")));
        assert!(five.iter().any(|(n, o)| n == "Power" && *o == 0x1000_0101));
        assert!(named_objects("ds100").is_empty());
        // Names are unique.
        let mut names: Vec<&String> = named.iter().map(|(n, _)| n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), named.len());
    }

    #[test]
    fn ports_by_model() {
        assert_eq!(default_port("d80"), Some(30013));
        assert_eq!(default_port("10d"), Some(30013));
        assert_eq!(default_port("d90"), Some(50014));
        assert_eq!(default_port("5dm"), Some(50014));
        assert_eq!(default_port("ds100"), None);
    }

    #[test]
    fn named_commands_become_aes70_commands() {
        assert_eq!(
            t(
                "d80",
                "set_channel_mute",
                json!({"channel": "B", "muted": true})
            ),
            (
                "set_mute",
                params(json!({"object": "B/Mute", "muted": true}))
            )
        );
        assert_eq!(
            t(
                "d40",
                "set_channel_delay",
                json!({"channel": "A", "delay_ms": 12.5})
            ),
            (
                "set_delay",
                params(json!({"object": "A/Delay", "delay_s": 0.0125}))
            )
        );
        assert_eq!(
            t("d40", "set_power", json!({"powered": false})),
            (
                "set_switch",
                params(json!({"object": "Power", "position": 0}))
            )
        );
        assert_eq!(
            t("d80", "set_input_fallback", json!({"mode": "auto"})),
            (
                "set_switch",
                params(json!({"object": "InputFallback/Mode", "position": 2}))
            )
        );
        assert_eq!(
            t(
                "10d",
                "set_input_routing",
                json!({"channel": "C", "input": "D2", "enabled": true})
            ),
            (
                "set_switch",
                params(json!({"object": "C/Input D2", "position": 1}))
            )
        );
        assert_eq!(
            t(
                "d80",
                "set_channel_filter",
                json!({"channel": "A", "filter": 3, "value": -2})
            ),
            (
                "set_int",
                params(json!({"object": "A/Filter3", "value": -2}))
            )
        );
        assert_eq!(
            t("d80", "load_amp_preset", json!({"preset": 5})),
            (
                "call_method",
                params(
                    json!({"ono": 10003, "method": "3.3", "param_count": 1, "params_hex": "05"})
                )
            )
        );
        assert_eq!(
            t(
                "d40",
                "backup_and_load_amp_preset",
                json!({"preset": 2, "backup": 14})
            ),
            (
                "call_method",
                params(
                    json!({"ono": 10003, "method": "3.1", "param_count": 2, "params_hex": "020e"})
                )
            )
        );
        assert_eq!(
            t("d80", "identify", json!({"enabled": true})),
            (
                "call_method",
                params(
                    json!({"ono": 0x1000_0327u32, "method": "5.2", "param_count": 1, "params_hex": "0200"})
                )
            )
        );
        assert_eq!(
            t("d80", "set_device_time", json!({"unix_seconds": 0})),
            (
                "call_method",
                params(
                    json!({"ono": 9, "method": "3.2", "param_count": 1, "params_hex": "83aa7e8000000000"})
                )
            )
        );
        // Not for every model.
        assert!(matches!(
            translate("5d", "load_amp_preset", &params(json!({"preset": 1})), 0),
            Err(CommandError::UnsupportedForModel { .. })
        ));
        assert!(matches!(
            translate("d40", "set_input_mode", &params(json!({"mode": 1})), 0),
            Err(CommandError::UnsupportedForModel { .. })
        ));
        // A DS100 identifies as any AES70 device; generic commands pass.
        assert!(translate("ds100", "identify", &params(json!({})), 0)
            .unwrap()
            .is_none());
        assert!(translate(
            "d80",
            "set_gain",
            &params(json!({"ono": 5, "gain_db": 0})),
            0
        )
        .unwrap()
        .is_none());
    }
}
