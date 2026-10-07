//! Fairlight EQ, dynamics, input settings, audio routing, monitoring and
//! solo; classic audio monitoring, headphones and audio-follow-video.
//!
//! Layouts from Sofie `commands/Fairlight/` and `commands/Audio/`. Gains,
//! thresholds and ranges are in hundredths of a dB, ratios in hundredths,
//! attack, hold and release in hundredths of a millisecond, EQ frequencies in
//! hertz and Q in hundredths: the LibAtem samples in Sofie
//! `commands/__tests__/libatem-data.json` give each field both as bytes and
//! in those units. Classic audio gains use the classic dB conversion.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::{
    classic_db, classic_raw, fields, fits, hundredths, i16_at, i64_at, int, invalid, one, opt_bool,
    opt_int, opt_num, put16, put32, text, u16_at, u32_at, unsupported, Atem, Body, CommandError,
    Out, Params, V2_30, W,
};

/// External port types, by bit (Sofie `enums/index.ts:115-130`).
const PORTS: [&str; 13] = [
    "sdi",
    "hdmi",
    "component",
    "composite",
    "svideo",
    "xlr",
    "aes_ebu",
    "rca",
    "internal",
    "ts_jack",
    "madi",
    "trs_jack",
    "rj45",
];

/// EQ band shapes, by bit (OpenSwitcher AEBP "Band filter").
pub(super) const SHAPES: [&str; 6] = [
    "low_shelf",
    "low_pass",
    "bell",
    "notch",
    "high_pass",
    "high_shelf",
];

/// Fairlight input configurations and analogue input levels, by bit (Sofie
/// `enums/index.ts:347-358`).
pub(super) const CONFIGURATIONS: [&str; 3] = ["mono", "stereo", "dual_mono"];
pub(super) const INPUT_LEVELS: [&str; 3] = ["microphone", "consumer_line", "pro_line"];

/// Fairlight input types (Sofie `enums/index.ts:365-370`).
fn input_type(v: u8) -> &'static str {
    match v {
        0 => "embedded_with_video",
        1 => "media_player",
        2 => "audio_in",
        4 => "madi",
        _ => "unknown",
    }
}

/// Internal audio port types (Sofie `enums/index.ts:407-420`, several
/// marked there as unverified).
const INTERNAL_PORTS: [&str; 12] = [
    "not_internal",
    "no_audio",
    "talkback_mix",
    "engineering_talkback_mix",
    "production_talkback_mix",
    "media_player",
    "program",
    "return",
    "monitor",
    "madi",
    "aux_out",
    "audio_aux_out",
];

/// Audio channel pairs (Sofie `enums/index.ts:396-405`).
const PAIRS: [&str; 8] = [
    "1_2", "3_4", "5_6", "7_8", "9_10", "11_12", "13_14", "15_16",
];

/// An external port type's name; the value is a single bit.
pub(super) fn port_name(v: u16) -> &'static str {
    if v == 0 || !v.is_power_of_two() {
        return "unknown";
    }
    PORTS
        .get(v.trailing_zeros() as usize)
        .copied()
        .unwrap_or("unknown")
}

/// The names of the bits set in `bits`.
fn bit_names(bits: u8, names: &[&'static str]) -> Vec<&'static str> {
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| bits & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect()
}

/// A single-bit value's name.
fn bit_name(v: u8, names: &[&'static str]) -> &'static str {
    if v == 0 || !v.is_power_of_two() {
        return "unknown";
    }
    names
        .get(v.trailing_zeros() as usize)
        .copied()
        .unwrap_or("unknown")
}

/// The bit for a name in `names`, from an enum parameter.
fn bit_param(params: &Params, name: &str, names: &[&str]) -> Result<Option<u8>, CommandError> {
    Ok(super::enum_param(params, name, names)?.map(|i| 1u8 << i))
}

/// Meter levels in hundredths of a dB: a source's (from `at`: input, the
/// expander, compressor and limiter gain reductions, after dynamics, after
/// the fader) or the master's (no expander).
pub(super) fn levels(b: &[u8], at: usize, expander: bool) -> Value {
    let mut names: Vec<&str> = vec![
        "input_left",
        "input_right",
        "input_left_peak",
        "input_right_peak",
    ];
    if expander {
        names.push("expander_reduction");
    }
    names.extend([
        "compressor_reduction",
        "limiter_reduction",
        "output_left",
        "output_right",
        "output_left_peak",
        "output_right_peak",
        "left",
        "right",
        "left_peak",
        "right_peak",
    ]);
    let mut m = Map::new();
    for (i, n) in names.iter().enumerate() {
        m.insert((*n).into(), json!(hundredths(i16_at(b, at + 2 * i) as i64)));
    }
    Value::Object(m)
}

/// An EQ band from `at`: enabled, supported shapes, shape, supported
/// frequency ranges, frequency range, then u32 frequency at `at + 7`, i32
/// gain at `at + 11`, i16 Q at `at + 15`.
fn band(b: &[u8], at: usize) -> Value {
    json!({
        "enabled": b[at] != 0,
        "shapes": bit_names(b[at + 1], &SHAPES),
        "shape": bit_name(b[at + 2], &SHAPES),
        "frequency_ranges": (0..8).filter(|i| b[at + 3] & (1 << i) != 0).map(|i| 1u8 << i).collect::<Vec<_>>(),
        "frequency_range": b[at + 4],
        "frequency": u32_at(b, at + 7),
        "gain": hundredths(super::i32_at(b, at + 11) as i64),
        "q": hundredths(i16_at(b, at + 15) as i64),
    })
}

/// Dynamics fields from a body: (name, offset, width).
fn dynamics(b: &[u8], enabled: usize, list: &[(&str, usize, W)]) -> Value {
    let mut m = fields(
        b,
        &list
            .iter()
            .map(|&(n, at, w)| (n, at, w, 100.0))
            .collect::<Vec<_>>(),
    );
    m.insert("enabled".into(), json!(b[enabled] != 0));
    Value::Object(m)
}

/// Compressor: threshold, ratio, attack, hold, release at these offsets
/// from the enabled flag (AICP from 16, MOCP from 0).
const COMPRESSOR: [(&str, usize, W); 5] = [
    ("threshold", 4, W::I32),
    ("ratio", 8, W::I16),
    ("attack", 12, W::I32),
    ("hold", 16, W::I32),
    ("release", 20, W::I32),
];
const LIMITER: [(&str, usize, W); 4] = [
    ("threshold", 4, W::I32),
    ("attack", 8, W::I32),
    ("hold", 12, W::I32),
    ("release", 16, W::I32),
];
const EXPANDER: [(&str, usize, W); 6] = [
    ("threshold", 4, W::I32),
    ("range", 8, W::I16),
    ("ratio", 10, W::I16),
    ("attack", 12, W::I32),
    ("hold", 16, W::I32),
    ("release", 20, W::I32),
];

fn shifted(list: &[(&'static str, usize, W)], by: usize) -> Vec<(&'static str, usize, W)> {
    list.iter().map(|&(n, at, w)| (n, at + by, w)).collect()
}

/// A Fairlight source's path in the state.
fn source_patch(input: u16, source: i64, v: Value) -> Value {
    json!({"fairlight": {"inputs": {input.to_string(): {"sources": {source.to_string(): v}}}}})
}

/// A routing id: the output or source in the high 16 bits, the channel pair
/// in the low.
fn routing(id: u32, external: u16, internal: u16, name: &[u8]) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(id >> 16));
    m.insert(
        "channel_pair".into(),
        json!(super::named(&PAIRS, (id & 0xFFFF) as u8)),
    );
    m.insert("external_port".into(), json!(port_name(external)));
    m.insert(
        "internal_port".into(),
        json!(super::named(&INTERNAL_PORTS, internal as u8)),
    );
    m.insert("name".into(), json!(text(name)));
    m
}

impl Atem {
    pub(super) fn decode_audio(&mut self, name: &[u8; 4], b: &[u8]) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        match name {
            // u16 input, rsv, i64 source at 8, band at 16, then the band
            // (Sofie `Fairlight/FairlightMixerSourceEqualizerBandCommand.ts:66-80`).
            b"AEBP" => {
                need(34)?;
                let band_no = (b[16] as u32 + 1).to_string();
                Some(source_patch(
                    u16_at(b, 0),
                    i64_at(b, 8),
                    json!({"equalizer": {"bands": {band_no: band(b, 17)}}}),
                ))
            }
            // Band, then the band (Sofie
            // `Fairlight/FairlightMixerMasterEqualizerBandCommand.ts:55-68`).
            b"AMBP" => {
                need(18)?;
                let band_no = (b[0] as u32 + 1).to_string();
                Some(json!({"fairlight": {"master": {"equalizer": {"bands": {
                    band_no: band(b, 1),
                }}}}}))
            }
            // Input, source, enabled at 16, then the compressor (Sofie
            // `Fairlight/FairlightMixerSourceCompressorCommand.ts:60-72`).
            b"AICP" => {
                need(40)?;
                let c = dynamics(b, 16, &shifted(&COMPRESSOR, 16));
                Some(source_patch(
                    u16_at(b, 0),
                    i64_at(b, 8),
                    json!({"dynamics": {"compressor": c}}),
                ))
            }
            b"AILP" => {
                need(36)?;
                let l = dynamics(b, 16, &shifted(&LIMITER, 16));
                Some(source_patch(
                    u16_at(b, 0),
                    i64_at(b, 8),
                    json!({"dynamics": {"limiter": l}}),
                ))
            }
            // Expander enabled at 16, gate at 17 (Sofie
            // `Fairlight/FairlightMixerSourceExpanderCommand.ts:66-80`).
            b"AIXP" => {
                need(40)?;
                let mut x = dynamics(b, 16, &shifted(&EXPANDER, 16));
                x["gate"] = json!(b[17] != 0);
                Some(source_patch(
                    u16_at(b, 0),
                    i64_at(b, 8),
                    json!({"dynamics": {"expander": x}}),
                ))
            }
            b"MOCP" => {
                need(24)?;
                Some(json!({"fairlight": {"master": {"dynamics": {
                    "compressor": dynamics(b, 0, &COMPRESSOR),
                }}}}))
            }
            b"AMLP" => {
                need(20)?;
                Some(json!({"fairlight": {"master": {"dynamics": {
                    "limiter": dynamics(b, 0, &LIMITER),
                }}}}))
            }
            // Audio follow video on transitions (Sofie
            // `Fairlight/FairlightMixerMasterPropertiesCommand.ts:30-35`).
            b"FMPP" => {
                need(1)?;
                Some(json!({"fairlight": {"audio_follow_video": b[0] != 0}}))
            }
            // i32 gain, i32 master gain, master on at 8, i32 talkback gain at
            // 12, talkback on at 16, i32 sidetone gain at 28 (Sofie
            // `Fairlight/FairlightMixerMonitorCommand.ts:44-56`).
            b"FMHP" => {
                need(32)?;
                Some(json!({"fairlight": {"monitor": {
                    "gain": hundredths(super::i32_at(b, 0) as i64),
                    "master_gain": hundredths(super::i32_at(b, 4) as i64),
                    "master_muted": b[8] == 0,
                    "talkback_gain": hundredths(super::i32_at(b, 12) as i64),
                    "talkback_muted": b[16] == 0,
                    "sidetone_gain": hundredths(super::i32_at(b, 28) as i64),
                }}}))
            }
            // Solo on, u16 input at 8, i64 source at 16 (Sofie
            // `Fairlight/FairlightMixerMonitorSoloCommand.ts:40-48`).
            b"FAMS" => {
                need(24)?;
                Some(json!({"fairlight": {"solo": {
                    "enabled": b[0] == 1,
                    "input": u16_at(b, 8),
                    "source": i64_at(b, 16),
                }}}))
            }
            // u16 input, input type, 3 rsv, u16 external port, then before
            // 2.30 rca-to-xlr supported and on at 8 and 9, configurations at
            // 11 and 12; from 2.30 configurations at 9 and 10, input levels at
            // 11 and 12 (Sofie `Fairlight/FairlightMixerInputCommand.ts:76-102`,
            // which reads the old rca-to-xlr switch as pro line or microphone).
            b"FAIP" => {
                need(13)?;
                let input = u16_at(b, 0);
                self.topology.fairlight_inputs.insert(input);
                let mut m = json!({
                    "type": input_type(b[2]),
                    "external_port": port_name(u16_at(b, 6)),
                });
                if self.version >= V2_30 {
                    m["configurations"] = json!(bit_names(b[9], &CONFIGURATIONS));
                    m["configuration"] = json!(bit_name(b[10], &CONFIGURATIONS));
                    m["input_levels"] = json!(bit_names(b[11], &INPUT_LEVELS));
                    m["input_level"] = json!(bit_name(b[12], &INPUT_LEVELS));
                } else {
                    m["configurations"] = json!(bit_names(b[11], &CONFIGURATIONS));
                    m["configuration"] = json!(bit_name(b[12], &CONFIGURATIONS));
                    m["input_levels"] = if b[8] != 0 {
                        json!(["microphone", "pro_line"])
                    } else {
                        json!([])
                    };
                    m["input_level"] = json!(if b[9] != 0 { "pro_line" } else { "microphone" });
                }
                Some(json!({"fairlight": {"inputs": {input.to_string(): m}}}))
            }
            // A source removed, as when an input changes between stereo and
            // dual mono (Sofie `Fairlight/FairlightMixerSourceCommand.ts:8-30`).
            b"FASD" => {
                need(16)?;
                let (input, source) = (u16_at(b, 0), i64_at(b, 8));
                self.topology.fairlight_sources.remove(&(input, source));
                self.topology.fairlight_bands.remove(&(input, source));
                Some(source_patch(input, source, Value::Null))
            }
            // u32 id, u32 source, u16 external port, u16 internal port,
            // name[64] (Sofie `Fairlight/AudioRouting/AudioRoutingOutput.ts:40-55`).
            b"AROP" => {
                need(76)?;
                let id = u32_at(b, 0);
                self.topology.routing_outputs.insert(id);
                let mut m = routing(id, u16_at(b, 8), u16_at(b, 10), &b[12..76]);
                m.insert("source".into(), json!(u32_at(b, 4)));
                Some(json!({"fairlight": {"routing": {"outputs": {id.to_string(): m}}}}))
            }
            // u32 id, u16 external port, u16 internal port, name[64] (Sofie
            // `Fairlight/AudioRouting/AudioRoutingSource.ts:36-50`).
            b"ARSP" => {
                need(72)?;
                let id = u32_at(b, 0);
                self.topology.routing_sources.insert(id);
                let m = routing(id, u16_at(b, 4), u16_at(b, 6), &b[8..72]);
                Some(json!({"fairlight": {"routing": {"sources": {id.to_string(): m}}}}))
            }
            // Enabled, rsv, u16 gain, mute, solo, u16 solo input, dim, rsv,
            // u16 dim level in hundredths of a percent (Sofie
            // `Audio/AudioMixerMonitorCommand.ts:44-56`).
            b"AMmO" => {
                need(12)?;
                Some(json!({"audio": {"monitor": {
                    "enabled": b[0] != 0,
                    "gain": classic_db(u16_at(b, 2)),
                    "mute": b[4] != 0,
                    "solo": b[5] != 0,
                    "solo_input": u16_at(b, 6),
                    "dim": b[8] != 0,
                    "dim_level": hundredths(u16_at(b, 10) as i64),
                }}}))
            }
            // u16 gain, program out gain, talkback gain, sidetone gain (Sofie
            // `Audio/AudioMixerHeadphonesCommand.ts:36-44`).
            b"AMHP" => {
                need(8)?;
                Some(json!({"audio": {"headphones": {
                    "gain": classic_db(u16_at(b, 0)),
                    "program_out_gain": classic_db(u16_at(b, 2)),
                    "talkback_gain": classic_db(u16_at(b, 4)),
                    "sidetone_gain": classic_db(u16_at(b, 6)),
                }}}))
            }
            // Audio follow video on transitions (Sofie
            // `Audio/AudioMixerPropertiesCommand.ts:22-27`).
            b"AMPP" => {
                need(1)?;
                Some(json!({"audio": {"audio_follow_video": b[0] != 0}}))
            }
            _ => None,
        }
    }

    fn check_fairlight(&self, name: &str) -> Result<(), CommandError> {
        if !self.topology.fairlight {
            return Err(unsupported(name, "this switcher (no Fairlight mixer)"));
        }
        Ok(())
    }

    fn check_classic(&self, name: &str) -> Result<(), CommandError> {
        if !self.topology.classic_audio {
            return Err(unsupported(name, "this switcher (no classic audio mixer)"));
        }
        Ok(())
    }

    /// A Fairlight (input, source) the switcher reported.
    fn check_fairlight_source(
        &self,
        name: &str,
        params: &Params,
    ) -> Result<(u16, i64), CommandError> {
        self.check_fairlight(name)?;
        let input = int(params, "input");
        let source = int(params, "source");
        if !self
            .topology
            .fairlight_sources
            .contains_key(&(input as u16, source))
        {
            return Err(invalid(format!(
                "the Fairlight mixer has no source {source} on input {input}"
            )));
        }
        Ok((input as u16, source))
    }

    /// The index of an EQ band numbered from 1, of `count`.
    fn check_band(params: &Params, count: u8) -> Result<u8, CommandError> {
        let band = int(params, "band");
        if band < 1 || band > count as i64 {
            return Err(invalid(format!(
                "band {band} does not exist; the equalizer has {count}"
            )));
        }
        Ok((band - 1) as u8)
    }

    pub(super) fn build_audio(
        &self,
        name: &str,
        params: &Params,
    ) -> Result<Option<Out>, CommandError> {
        // A body with the input at 2 and the source at 8, as Fairlight source
        // commands share.
        let sourced = |len: usize, input: u16, source: i64| {
            let mut body = Body::new(params, len);
            body.b[2..4].copy_from_slice(&input.to_be_bytes());
            body.b[8..16].copy_from_slice(&source.to_be_bytes());
            body
        };
        match name {
            // Mask, rsv, u16 input, then before 2.30 rca-to-xlr (bit 0) and
            // the configuration (bit 1); from 2.30 the configuration (bit 0)
            // and the input level (bit 1) (Sofie
            // `Fairlight/FairlightMixerInputCommand.ts:6-50`).
            "set_fairlight_input" => {
                self.check_fairlight(name)?;
                let input = int(params, "input");
                if !self.topology.fairlight_inputs.contains(&(input as u16)) {
                    return Err(invalid(format!("the Fairlight mixer has no input {input}")));
                }
                let mut b = vec![0u8; 8];
                put16(&mut b, 2, input as u16);
                let config = bit_param(params, "configuration", &CONFIGURATIONS)?;
                let level = bit_param(params, "input_level", &INPUT_LEVELS)?;
                if self.version >= V2_30 {
                    if let Some(c) = config {
                        b[0] |= 1;
                        b[4] = c;
                    }
                    if let Some(l) = level {
                        b[0] |= 2;
                        b[5] = l;
                    }
                } else {
                    if let Some(l) = level {
                        // The old switch: on is pro line, off microphone.
                        let on = match l {
                            4 => 1,
                            1 => 0,
                            _ => return Err(invalid(
                                "before protocol 2.30 the input level is microphone or pro_line"
                                    .into(),
                            )),
                        };
                        b[0] |= 1;
                        b[4] = on;
                    }
                    if let Some(c) = config {
                        b[0] |= 2;
                        b[5] = c;
                    }
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CFIP", b)
            }
            // Mask, rsv, u16 input, 4 rsv, i64 source, band, enabled, shape,
            // frequency range, u32 frequency, i32 gain, i16 Q (Sofie
            // `Fairlight/FairlightMixerSourceEqualizerBandCommand.ts:9-46`).
            "set_fairlight_eq_band" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let count = self
                    .topology
                    .fairlight_bands
                    .get(&(input, source))
                    .copied()
                    .unwrap_or(0);
                let band_index = Self::check_band(params, count)?;
                let mut body = sourced(32, input, source);
                body.at(16, band_index);
                eq_fields(&mut body, 17)?;
                one(b"CEBP", body.done(0, W::U8)?)
            }
            // Mask, band, enabled, shape, frequency range, 3 rsv, u32
            // frequency, i32 gain, i16 Q (Sofie
            // `Fairlight/FairlightMixerMasterEqualizerBandCommand.ts:9-40`).
            "set_fairlight_master_eq_band" => {
                self.check_fairlight(name)?;
                let band_index = Self::check_band(params, self.topology.master_bands)?;
                let mut body = Body::new(params, 20);
                body.at(1, band_index);
                eq_fields(&mut body, 2)?;
                one(b"CMBP", body.done(0, W::U8)?)
            }
            "set_fairlight_compressor" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut body = sourced(40, input, source);
                dynamics_fields(&mut body, 16, &COMPRESSOR, 16);
                one(b"CICP", body.done(0, W::U8)?)
            }
            "set_fairlight_limiter" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut body = sourced(36, input, source);
                dynamics_fields(&mut body, 16, &LIMITER, 16);
                one(b"CILP", body.done(0, W::U8)?)
            }
            // Mask bits 0 enabled, 1 gate, 2 threshold, 3 range, 4 ratio, 5
            // attack, 6 hold, 7 release (Sofie
            // `Fairlight/FairlightMixerSourceExpanderCommand.ts:9-48`).
            "set_fairlight_expander" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut body = sourced(40, input, source);
                body.flag("enabled", 0, 16).flag("gate", 1, 17);
                for (i, &(field, at, w)) in EXPANDER.iter().enumerate() {
                    body.num(field, 2 + i as u32, 16 + at, 100.0, w);
                }
                one(b"CIXP", body.done(0, W::U8)?)
            }
            "set_fairlight_master_compressor" => {
                self.check_fairlight(name)?;
                let mut body = Body::new(params, 24);
                dynamics_fields(&mut body, 1, &COMPRESSOR, 0);
                one(b"CMCP", body.done(0, W::U8)?)
            }
            "set_fairlight_master_limiter" => {
                self.check_fairlight(name)?;
                let mut body = Body::new(params, 20);
                dynamics_fields(&mut body, 1, &LIMITER, 0);
                one(b"CMLP", body.done(0, W::U8)?)
            }
            // Mask (bit 0 whole EQ, 1 one band), rsv, u16 input, 4 rsv, i64
            // source, whole EQ, band (Sofie
            // `Fairlight/FairlightMixerSourceEqualizerResetCommand.ts:6-30`).
            "reset_fairlight_eq" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut b = vec![0u8; 20];
                put16(&mut b, 2, input);
                b[8..16].copy_from_slice(&source.to_be_bytes());
                if opt_int(params, "band").is_some() {
                    let count = self
                        .topology
                        .fairlight_bands
                        .get(&(input, source))
                        .copied()
                        .unwrap_or(0);
                    b[0] = 2;
                    b[17] = Self::check_band(params, count)?;
                } else {
                    b[0] = 1;
                    b[16] = 1;
                }
                one(b"RICE", b)
            }
            // Mask (bit 0 whole EQ, 1 one band), whole EQ, band, rsv (Sofie
            // `Fairlight/FairlightMixerMasterEqualizerResetCommand.ts:6-24`).
            "reset_fairlight_master_eq" => {
                self.check_fairlight(name)?;
                if opt_int(params, "band").is_some() {
                    let band_index = Self::check_band(params, self.topology.master_bands)?;
                    one(b"RMOE", vec![2, 0, band_index, 0])
                } else {
                    one(b"RMOE", vec![1, 1, 0, 0])
                }
            }
            // u16 input, 6 rsv, i64 source, rsv, bits at 17: 0 dynamics, 1
            // expander, 2 compressor, 3 limiter (Sofie
            // `Fairlight/FairlightMixerSourceDynamicsResetCommand.ts:6-38`).
            "reset_fairlight_dynamics" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut b = vec![0u8; 20];
                put16(&mut b, 0, input);
                b[8..16].copy_from_slice(&source.to_be_bytes());
                b[17] = parts(params, &["dynamics", "expander", "compressor", "limiter"])?;
                one(b"RICD", b)
            }
            // rsv, bits: 0 dynamics, 1 expander, 2 compressor, 3 limiter, 2
            // rsv (Sofie `Fairlight/FairlightMixerMasterDynamicsResetCommand.ts:6-32`).
            "reset_fairlight_master_dynamics" => {
                self.check_fairlight(name)?;
                let bits = parts(params, &["dynamics", "expander", "compressor", "limiter"])?;
                one(b"RMOD", vec![0, bits, 0, 0])
            }
            // Bits (0 all, 1 master); with all, byte 1 is 1; with master,
            // byte 3 is 4, values Sofie found the switcher needs (Sofie
            // `Fairlight/FairlightMixerResetPeakLevelsCommand.ts:6-26`).
            "reset_fairlight_peaks" => {
                self.check_fairlight(name)?;
                let mut b = vec![0u8; 4];
                if opt_bool(params, "all") == Some(true) {
                    b[0] |= 1;
                    b[1] = 1;
                }
                if opt_bool(params, "master") == Some(true) {
                    b[0] |= 2;
                    b[3] = 4;
                }
                if b[0] == 0 {
                    return Err(invalid("give all or master".into()));
                }
                one(b"RFLP", b)
            }
            // u16 input, 6 rsv, i64 source, rsv, bits at 17: 0 dynamics
            // input, 1 dynamics output, 2 output (Sofie
            // `Fairlight/FairlightMixerSourceResetPeakLevelsCommand.ts:6-36`).
            "reset_fairlight_source_peaks" => {
                let (input, source) = self.check_fairlight_source(name, params)?;
                let mut b = vec![0u8; 20];
                put16(&mut b, 0, input);
                b[8..16].copy_from_slice(&source.to_be_bytes());
                b[17] = parts(params, &["dynamics_input", "dynamics_output", "output"])?;
                one(b"RFIP", b)
            }
            // Mask, audio follow video, 2 rsv (Sofie
            // `Fairlight/FairlightMixerMasterPropertiesCommand.ts:6-20`).
            "set_fairlight_audio_follow_video" => {
                self.check_fairlight(name)?;
                one(b"CMPP", vec![1, super::flag(params, "enabled") as u8, 0, 0])
            }
            // Mask (0 gain, 1 master gain, 2 master on, 3 talkback gain, 4
            // talkback on, 7 sidetone gain), 3 rsv, i32 gain, i32 master
            // gain, master on, 3 rsv, i32 talkback gain, talkback on, 11
            // rsv, i32 sidetone gain (Sofie `Fairlight/FairlightMixerMonitorCommand.ts:6-34`).
            // The "on" bytes are the inverse of muted.
            "set_fairlight_monitor" => {
                self.check_fairlight(name)?;
                let mut body = Body::new(params, 36);
                body.num("gain", 0, 4, 100.0, W::I32)
                    .num("master_gain", 1, 8, 100.0, W::I32)
                    .num("talkback_gain", 3, 16, 100.0, W::I32)
                    .num("sidetone_gain", 7, 32, 100.0, W::I32);
                for (field, bit, at) in [("master_muted", 2, 12), ("talkback_muted", 4, 20)] {
                    if let Some(muted) = opt_bool(params, field) {
                        body.mask |= 1 << bit;
                        body.b[at] = !muted as u8;
                    }
                }
                one(b"CFMH", body.done(0, W::U8)?)
            }
            // Mask (0 solo, 1 the soloed source), solo, 6 rsv, u16 input, 6
            // rsv, i64 source (Sofie `Fairlight/FairlightMixerMonitorSoloCommand.ts:6-30`).
            "set_fairlight_solo" => {
                self.check_fairlight(name)?;
                let mut b = vec![0u8; 24];
                if let Some(v) = opt_bool(params, "enabled") {
                    b[0] |= 1;
                    b[1] = v as u8;
                }
                if opt_int(params, "input").is_some() || opt_int(params, "source").is_some() {
                    let (input, source) = self.check_fairlight_source(name, params)?;
                    b[0] |= 2;
                    put16(&mut b, 8, input);
                    b[16..24].copy_from_slice(&source.to_be_bytes());
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CFMS", b)
            }
            // Mask (0 source, 1 name), 3 rsv, u32 output id, u32 source id,
            // name[64] (Sofie `Fairlight/AudioRouting/AudioRoutingOutput.ts:6-30`).
            "set_audio_routing_output" => {
                self.check_fairlight(name)?;
                let output =
                    routing_id(params, "output", &self.topology.routing_outputs, "output")?;
                let mut b = vec![0u8; 76];
                put32(&mut b, 4, output);
                if opt_int(params, "source").is_some() {
                    let source =
                        routing_id(params, "source", &self.topology.routing_sources, "source")?;
                    b[0] |= 1;
                    put32(&mut b, 8, source);
                }
                if let Some(n) = fits(params, "name", 63)? {
                    b[0] |= 2;
                    b[12..12 + n.len()].copy_from_slice(n.as_bytes());
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"AROC", b)
            }
            // Mask (0 name), 3 rsv, u32 source id, name[64] (Sofie
            // `Fairlight/AudioRouting/AudioRoutingSource.ts:6-28`).
            "set_audio_routing_source" => {
                self.check_fairlight(name)?;
                let source =
                    routing_id(params, "source", &self.topology.routing_sources, "source")?;
                let n = fits(params, "name", 63)?.unwrap_or_default();
                let mut b = vec![0u8; 72];
                b[0] = 1;
                put32(&mut b, 4, source);
                b[8..8 + n.len()].copy_from_slice(n.as_bytes());
                one(b"ARSC", b)
            }
            // Mask (0 on, 1 gain, 2 mute, 3 solo, 4 solo input, 5 dim, 6 dim
            // level), on, u16 gain, mute, solo, u16 solo input, dim, rsv, u16
            // dim level (Sofie `Audio/AudioMixerMonitorCommand.ts:6-33`).
            "set_audio_monitor" => {
                self.check_classic(name)?;
                let mut body = Body::new(params, 12);
                body.flag("enabled", 0, 1)
                    .flag("mute", 2, 4)
                    .flag("solo", 3, 5)
                    .flag("dim", 5, 8)
                    .num("dim_level", 6, 10, 100.0, W::U16);
                if let Some(db) = opt_num(params, "gain") {
                    body.mask |= 1 << 1;
                    put16(&mut body.b, 2, classic_raw(db));
                }
                if let Some(input) = opt_int(params, "solo_input") {
                    if !self.topology.classic_inputs.contains(&(input as u16)) {
                        return Err(invalid(format!("the audio mixer has no input {input}")));
                    }
                    body.mask |= 1 << 4;
                    put16(&mut body.b, 6, input as u16);
                }
                one(b"CAMm", body.done(0, W::U8)?)
            }
            // Mask (0 gain, 1 program out, 2 talkback, 3 sidetone), rsv, then
            // four u16 gains (Sofie `Audio/AudioMixerHeadphonesCommand.ts:6-28`).
            "set_audio_headphones" => {
                self.check_classic(name)?;
                let mut b = vec![0u8; 12];
                for (i, field) in ["gain", "program_out_gain", "talkback_gain", "sidetone_gain"]
                    .iter()
                    .enumerate()
                {
                    if let Some(db) = opt_num(params, field) {
                        b[0] |= 1 << i;
                        put16(&mut b, 2 + 2 * i, classic_raw(db));
                    }
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CAMH", b)
            }
            // Mask always 1, audio follow video, 2 rsv (Sofie
            // `Audio/AudioMixerPropertiesCommand.ts:6-16`).
            "set_audio_follow_video" => {
                self.check_classic(name)?;
                one(b"CAMP", vec![1, super::flag(params, "enabled") as u8, 0, 0])
            }
            // Mask (0 all, 1 input, 2 master, 3 monitor), all, u16 input,
            // master, monitor, 2 rsv (Sofie `Audio/AudioMixerResetPeaksCommand.ts:15-35`).
            "reset_audio_peaks" => {
                self.check_classic(name)?;
                let mut b = vec![0u8; 8];
                if opt_bool(params, "all") == Some(true) {
                    b[0] |= 1;
                    b[1] = 1;
                }
                if let Some(input) = opt_int(params, "input") {
                    if !self.topology.classic_inputs.contains(&(input as u16)) {
                        return Err(invalid(format!("the audio mixer has no input {input}")));
                    }
                    b[0] |= 2;
                    put16(&mut b, 2, input as u16);
                }
                for (bit, field, at) in [(2, "master", 4), (3, "monitor", 5)] {
                    if opt_bool(params, field) == Some(true) {
                        b[0] |= 1 << bit;
                        b[at] = 1;
                    }
                }
                if b[0] == 0 {
                    return Err(invalid("give at least one meter to reset".into()));
                }
                one(b"RAMP", b)
            }
            _ => Ok(None),
        }
    }
}

/// The band's settings from `at`: enabled (bit 0), shape (bit 1), frequency
/// range (bit 2) as single bytes, then u32 frequency (bit 3) at `at + 3`
/// rounded to the next 4, i32 gain (bit 4), i16 Q (bit 5).
fn eq_fields(body: &mut Body, at: usize) -> Result<(), CommandError> {
    body.flag("enabled", 0, at);
    if let Some(shape) = bit_param(body.params, "shape", &SHAPES)? {
        body.mask |= 1 << 1;
        body.b[at + 1] = shape;
    }
    if let Some(range) = opt_int(body.params, "frequency_range") {
        body.mask |= 1 << 2;
        body.b[at + 2] = range as u8;
    }
    let f = (at + 3).next_multiple_of(4);
    body.num("frequency", 3, f, 1.0, W::U32)
        .num("gain", 4, f + 4, 100.0, W::I32)
        .num("q", 5, f + 8, 100.0, W::I16);
    Ok(())
}

/// A compressor's or limiter's settings: enabled at `enabled` (bit 0), then
/// the fields, each at its offset plus `from`, in hundredths (bits 1 on).
fn dynamics_fields(body: &mut Body, enabled: usize, list: &[(&str, usize, W)], from: usize) {
    body.flag("enabled", 0, enabled);
    for (i, &(field, at, w)) in list.iter().enumerate() {
        body.num(field, 1 + i as u32, from + at, 100.0, w);
    }
}

/// Bits for the boolean parameters given true, in order.
fn parts(params: &Params, names: &[&str]) -> Result<u8, CommandError> {
    let bits = names
        .iter()
        .enumerate()
        .filter(|(_, n)| opt_bool(params, n) == Some(true))
        .fold(0u8, |b, (i, _)| b | 1 << i);
    if bits == 0 {
        return Err(invalid(format!(
            "give at least one of {}",
            names.join(", ")
        )));
    }
    Ok(bits)
}

/// A routing output or source id the switcher reported.
fn routing_id(
    params: &Params,
    name: &str,
    known: &BTreeSet<u32>,
    what: &str,
) -> Result<u32, CommandError> {
    let id = int(params, name);
    if !(0..=u32::MAX as i64).contains(&id) || !known.contains(&(id as u32)) {
        return Err(invalid(format!(
            "the switcher has no audio routing {what} {id}"
        )));
    }
    Ok(id as u32)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cmd, decoded, dump, hex, payload, ready_with, refused};
    use super::super::{command, Atem, CommandError};
    use serde_json::{json, Value};

    const SOURCE: [u8; 8] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00];
    const OUTPUT_ID: u32 = 0x0001_0000;
    const SOURCE_ID: u32 = 0x0002_0001;

    /// The test switcher, its Fairlight source with six EQ bands, a master
    /// with six, Fairlight input 1, and one routing output and source.
    fn fairlight() -> Atem {
        let mut d = dump();
        let fasp = d.iter().position(|c| &c[4..8] == b"FASP").unwrap();
        d[fasp][8 + 28] = 6;
        let mut famp = vec![0u8; 20];
        famp[0] = 6;
        let mut arop = vec![0u8; 76];
        arop[..4].copy_from_slice(&OUTPUT_ID.to_be_bytes());
        let mut arsp = vec![0u8; 72];
        arsp[..4].copy_from_slice(&SOURCE_ID.to_be_bytes());
        let end = d.len() - 1;
        d.insert(end, cmd(b"FAMP", &famp));
        d.insert(
            end,
            cmd(
                b"FAIP",
                &[0, 1, 2, 0, 0, 0, 0, 0x20, 0, 7, 2, 5, 4, 0, 0, 0],
            ),
        );
        d.insert(end, cmd(b"AROP", &arop));
        d.insert(end, cmd(b"ARSP", &arsp));
        ready_with(d)
    }

    /// The test switcher with the classic mixer and its input 1.
    fn classic() -> Atem {
        let mut d = dump();
        let fac = d.iter().position(|c| &c[4..8] == b"_FAC").unwrap();
        d[fac] = cmd(b"_AMC", &[8, 1, 0, 0]);
        let fa = d.iter().position(|c| &c[4..8] == b"FASP").unwrap();
        d[fa] = cmd(
            b"AMIP",
            &[0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0x80, 0, 0, 0, 0, 0],
        );
        ready_with(d)
    }

    /// A Fairlight source command body: mask, input 1 at 2, the source at 8.
    fn sourced(len: usize, mask: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; len];
        b[..mask.len()].copy_from_slice(mask);
        b[3] = 1;
        b[8..16].copy_from_slice(&SOURCE);
        b
    }

    #[test]
    fn fairlight_eq_and_dynamics_layouts() {
        let mut m = fairlight();
        let src = json!({"input": 1, "source": -65280});
        let with = |extra: Value| {
            let mut p = src.clone();
            p.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            p
        };
        // LibAtem's CEBP sample: gain -15.22 dB is FA0E at 24.
        let p = payload(
            &mut m,
            "set_fairlight_eq_band",
            with(json!({"band": 4, "gain": -15.22})),
        );
        let mut b = sourced(32, &[0x10]);
        b[16] = 3;
        b[24..28].copy_from_slice(&[0xFF, 0xFF, 0xFA, 0x0E]);
        assert_eq!(p, command(b"CEBP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_eq_band",
            with(
                json!({"band": 1, "enabled": true, "shape": "bell", "frequency_range": 4,
                        "frequency": 6315, "q": 6.75}),
            ),
        );
        let mut b = sourced(32, &[0x2F]);
        b[17..20].copy_from_slice(&[1, 4, 4]);
        b[20..24].copy_from_slice(&6315u32.to_be_bytes());
        b[28..30].copy_from_slice(&[0x02, 0xA3]);
        assert_eq!(p, command(b"CEBP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_master_eq_band",
            json!({"band": 1, "shape": "low_pass", "frequency": 6567}),
        );
        let mut b = vec![0u8; 20];
        b[0] = 0x0A;
        b[3] = 2;
        b[8..12].copy_from_slice(&6567u32.to_be_bytes());
        assert_eq!(p, command(b"CMBP", &b));
        // LibAtem's CICP sample: release 3752.6 ms is 0005B9DC.
        let p = payload(
            &mut m,
            "set_fairlight_compressor",
            with(json!({"enabled": true, "release": 3752.6})),
        );
        let mut b = sourced(40, &[0x21]);
        b[16] = 1;
        b[36..40].copy_from_slice(&[0, 0x05, 0xB9, 0xDC]);
        assert_eq!(p, command(b"CICP", &b));
        // LibAtem's CIXP sample: range 23.99 dB is 095F, ratio 2.82 is 011A.
        let p = payload(
            &mut m,
            "set_fairlight_expander",
            with(json!({"gate": true, "range": 23.99, "ratio": 2.82})),
        );
        let mut b = sourced(40, &[0x1A]);
        b[17] = 1;
        b[24..28].copy_from_slice(&[0x09, 0x5F, 0x01, 0x1A]);
        assert_eq!(p, command(b"CIXP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_limiter",
            with(json!({"threshold": -5.34})),
        );
        let mut b = sourced(36, &[0x02]);
        b[20..24].copy_from_slice(&[0xFF, 0xFF, 0xFD, 0xEA]);
        assert_eq!(p, command(b"CILP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_master_compressor",
            json!({"ratio": 11.43, "hold": 3390.22}),
        );
        let mut b = vec![0u8; 24];
        b[0] = 0x14;
        b[8..10].copy_from_slice(&[0x04, 0x77]);
        b[16..20].copy_from_slice(&[0, 0x05, 0x2C, 0x4E]);
        assert_eq!(p, command(b"CMCP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_master_limiter",
            json!({"enabled": false, "attack": 9.23}),
        );
        let mut b = vec![0u8; 20];
        b[0] = 0x05;
        b[8..12].copy_from_slice(&[0, 0, 0x03, 0x9B]);
        assert_eq!(p, command(b"CMLP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_source",
            with(
                json!({"frames_delay": 3, "stereo_simulation": 50.0, "eq_enabled": true,
                        "eq_gain": -4.69, "make_up_gain": 0.3}),
            ),
        );
        let mut b = sourced(48, &[0x00, 0x3D]);
        b[16] = 3;
        b[24..26].copy_from_slice(&[0x13, 0x88]);
        b[26] = 1;
        b[28..32].copy_from_slice(&[0xFF, 0xFF, 0xFE, 0x2B]);
        b[32..36].copy_from_slice(&[0, 0, 0, 0x1E]);
        assert_eq!(p, command(b"CFSP", &b));
        // LibAtem's CFMP sample: EQ gain -8.26 dB and make-up 14.11 dB.
        let p = payload(
            &mut m,
            "set_fairlight_master",
            json!({"eq_enabled": true, "eq_gain": -8.26, "make_up_gain": 14.11}),
        );
        assert_eq!(
            p,
            command(
                b"CFMP",
                &hex("07-01-00-00-FF-FF-FC-C6-00-00-05-83-00-00-00-00-00-00-00-00")
            )
        );
    }

    #[test]
    fn fairlight_inputs_monitoring_routing_and_resets() {
        let mut m = fairlight();
        let p = payload(
            &mut m,
            "set_fairlight_input",
            json!({"input": 1, "configuration": "stereo", "input_level": "pro_line"}),
        );
        assert_eq!(p, command(b"CFIP", &[3, 0, 0, 1, 2, 4, 0, 0]));
        // LibAtem's CFMH sample: gain 5.81 dB is 0245, talkback -47.37 dB
        // FFFFED7F; the master "on" byte is the inverse of muted.
        let p = payload(
            &mut m,
            "set_fairlight_monitor",
            json!({"gain": 5.81, "master_muted": false, "talkback_gain": -47.37}),
        );
        let mut b = vec![0u8; 36];
        b[0] = 0x0D;
        b[4..8].copy_from_slice(&[0, 0, 0x02, 0x45]);
        b[12] = 1;
        b[16..20].copy_from_slice(&[0xFF, 0xFF, 0xED, 0x7F]);
        assert_eq!(p, command(b"CFMH", &b));
        let p = payload(
            &mut m,
            "set_fairlight_solo",
            json!({"enabled": true, "input": 1, "source": -65280}),
        );
        let mut b = vec![0u8; 24];
        b[..2].copy_from_slice(&[3, 1]);
        b[9] = 1;
        b[16..24].copy_from_slice(&SOURCE);
        assert_eq!(p, command(b"CFMS", &b));
        let p = payload(
            &mut m,
            "set_fairlight_audio_follow_video",
            json!({"enabled": true}),
        );
        assert_eq!(p, command(b"CMPP", &[1, 1, 0, 0]));
        let p = payload(
            &mut m,
            "set_audio_routing_output",
            json!({"output": OUTPUT_ID, "source": SOURCE_ID, "name": "Out"}),
        );
        let mut b = vec![0u8; 76];
        b[0] = 3;
        b[4..8].copy_from_slice(&OUTPUT_ID.to_be_bytes());
        b[8..12].copy_from_slice(&SOURCE_ID.to_be_bytes());
        b[12..15].copy_from_slice(b"Out");
        assert_eq!(p, command(b"AROC", &b));
        let p = payload(
            &mut m,
            "set_audio_routing_source",
            json!({"source": SOURCE_ID, "name": "Mic"}),
        );
        let mut b = vec![0u8; 72];
        b[0] = 1;
        b[4..8].copy_from_slice(&SOURCE_ID.to_be_bytes());
        b[8..11].copy_from_slice(b"Mic");
        assert_eq!(p, command(b"ARSC", &b));

        let p = payload(
            &mut m,
            "reset_fairlight_eq",
            json!({"input": 1, "source": -65280, "band": 2}),
        );
        let mut b = sourced(20, &[2]);
        b[17] = 1;
        assert_eq!(p, command(b"RICE", &b));
        let p = payload(
            &mut m,
            "reset_fairlight_eq",
            json!({"input": 1, "source": -65280}),
        );
        let mut b = sourced(20, &[1]);
        b[16] = 1;
        assert_eq!(p, command(b"RICE", &b));
        // RICD has the input at 0, not 2.
        let p = payload(
            &mut m,
            "reset_fairlight_dynamics",
            json!({"input": 1, "source": -65280, "compressor": true, "limiter": true}),
        );
        let mut b = vec![0u8; 20];
        b[1] = 1;
        b[8..16].copy_from_slice(&SOURCE);
        b[17] = 0x0C;
        assert_eq!(p, command(b"RICD", &b));
        let p = payload(&mut m, "reset_fairlight_master_eq", json!({}));
        assert_eq!(p, command(b"RMOE", &[1, 1, 0, 0]));
        let p = payload(&mut m, "reset_fairlight_master_eq", json!({"band": 3}));
        assert_eq!(p, command(b"RMOE", &[2, 0, 2, 0]));
        let p = payload(
            &mut m,
            "reset_fairlight_master_dynamics",
            json!({"dynamics": true, "expander": true, "compressor": true, "limiter": true}),
        );
        assert_eq!(p, command(b"RMOD", &[0, 0x0F, 0, 0]));
        let p = payload(&mut m, "reset_fairlight_peaks", json!({"master": true}));
        assert_eq!(p, command(b"RFLP", &[2, 0, 0, 4]));
        let p = payload(
            &mut m,
            "reset_fairlight_source_peaks",
            json!({"input": 1, "source": -65280, "dynamics_output": true}),
        );
        b[17] = 2;
        assert_eq!(p, command(b"RFIP", &b));
    }

    #[test]
    fn fairlight_commands_are_checked() {
        let mut m = fairlight();
        let bad = |m: &mut Atem, name: &str, params: Value| {
            assert!(
                matches!(refused(m, name, params), CommandError::InvalidParams { .. }),
                "{name}"
            )
        };
        bad(
            &mut m,
            "set_fairlight_eq_band",
            json!({"input": 1, "source": -65280, "band": 7, "gain": 0.0}),
        );
        bad(
            &mut m,
            "set_fairlight_compressor",
            json!({"input": 2, "source": -65280, "enabled": true}),
        );
        bad(
            &mut m,
            "set_fairlight_input",
            json!({"input": 2, "configuration": "mono"}),
        );
        bad(
            &mut m,
            "reset_fairlight_dynamics",
            json!({"input": 1, "source": -65280}),
        );
        bad(
            &mut m,
            "set_audio_routing_source",
            json!({"source": 9, "name": "X"}),
        );
        bad(&mut m, "reset_fairlight_peaks", json!({}));
        assert!(matches!(
            refused(&mut m, "set_audio_monitor", json!({"mute": true})),
            CommandError::UnsupportedForModel { .. }
        ));
        let mut m = classic();
        assert!(matches!(
            refused(&mut m, "set_fairlight_monitor", json!({"gain": 0.0})),
            CommandError::UnsupportedForModel { .. }
        ));
        bad(&mut m, "set_audio_monitor", json!({"solo_input": 2}));
        bad(&mut m, "reset_audio_peaks", json!({}));
    }

    #[test]
    fn classic_monitoring_layouts() {
        let mut m = classic();
        // LibAtem's CAMm sample: dim level 85.26 % is 214E.
        let p = payload(
            &mut m,
            "set_audio_monitor",
            json!({"mute": true, "solo": true, "solo_input": 1, "dim_level": 85.26}),
        );
        assert_eq!(
            p,
            command(b"CAMm", &hex("5C-00-00-00-01-01-00-01-00-00-21-4E"))
        );
        // -6 dB is 10^(-6/20) x 32768, floored: 16422.
        let p = payload(&mut m, "set_audio_monitor", json!({"gain": -6.0}));
        assert_eq!(
            p,
            command(b"CAMm", &[2, 0, 0x40, 0x26, 0, 0, 0, 0, 0, 0, 0, 0])
        );
        let p = payload(
            &mut m,
            "set_audio_headphones",
            json!({"talkback_gain": -6.0}),
        );
        assert_eq!(
            p,
            command(b"CAMH", &[4, 0, 0, 0, 0, 0, 0x40, 0x26, 0, 0, 0, 0])
        );
        let p = payload(&mut m, "set_audio_follow_video", json!({"enabled": false}));
        assert_eq!(p, command(b"CAMP", &[1, 0, 0, 0]));
        let p = payload(
            &mut m,
            "reset_audio_peaks",
            json!({"input": 1, "master": true}),
        );
        assert_eq!(p, command(b"RAMP", &[6, 0, 0, 1, 1, 0, 0, 0]));
        let p = payload(
            &mut m,
            "set_audio_master",
            json!({"follow_fade_to_black": true}),
        );
        assert_eq!(p, command(b"CAMM", &[4, 0, 0, 0, 0, 0, 1, 0]));
        let p = payload(
            &mut m,
            "set_audio_input",
            json!({"input": 1, "rca_to_xlr": true}),
        );
        assert_eq!(p, command(b"CAMI", &[8, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0]));
    }

    #[test]
    fn audio_state_from_the_libatem_samples() {
        let mut m = fairlight();
        let s = decoded(
            &mut m,
            &[
                cmd(b"AEBP", &hex("00-04-00-00-00-00-00-00-4C-9F-74-D8-71-F5-A0-C9-04-01-04-01-01-01-00-00-00-00-2B-97-FF-FF-FD-29-02-75-00-00")),
                cmd(b"AICP", &hex("04-4D-00-00-00-00-00-00-2A-3C-8D-5C-21-F4-A5-FE-00-00-00-00-FF-FF-FE-89-04-76-00-00-00-00-0D-FC-00-04-26-88-00-04-F4-1C")),
                cmd(b"AIXP", &hex("00-06-00-00-00-00-00-00-37-2B-97-3E-08-9F-FB-63-00-01-00-00-FF-FF-EF-22-04-0C-00-FD-00-00-19-B6-00-05-F9-06-00-04-45-C2")),
                cmd(b"AILP", &hex("07-D3-00-00-00-00-00-00-9F-A4-1D-AF-C3-F4-AB-76-01-00-00-00-FF-FF-FD-EA-00-00-07-23-00-05-69-8B-00-00-77-20")),
                cmd(b"MOCP", &hex("00-00-00-00-FF-FF-EE-33-04-51-00-00-00-00-1D-19-00-01-6F-2E-00-02-A6-23")),
                cmd(b"AMBP", &hex("00-01-20-02-08-08-00-00-00-00-19-A7-FF-FF-FB-FE-03-6B-00-00")),
                cmd(b"AMLP", &hex("01-00-00-00-FF-FF-FB-8F-00-00-03-9B-00-03-8E-56-00-05-C7-A9")),
                cmd(b"FMHP", &hex("FF-FF-FD-AE-FF-FF-F3-02-00-00-00-00-FF-FF-F8-5B-00-00-00-00-00-00-00-00-00-00-00-00-FF-FF-F1-E1")),
                cmd(b"FASP", &hex("00-01-00-00-00-00-00-00-5E-A6-47-1E-66-3D-0A-E3-01-E8-3B-00-FF-FF-DB-B2-00-00-18-C5-7C-01-00-00-FF-FF-FE-44-00-00-02-6D-1C-10-00-00-00-00-02-2C-02-04-00-00")),
                cmd(b"FAMP", &hex("AC-01-00-00-FF-FF-F9-FA-00-00-07-87-FF-FF-F1-3C-01-00-00-00")),
                cmd(b"FMPP", &[1, 0, 0, 0]),
                cmd(b"FAMS", &[1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00]),
                cmd(b"AROP", &hex("0F-E6-31-1E-15-F8-CB-EC-10-00-00-0B-61-39-31-39-61-36-39-33-2D-61-33-63-33-2D-34-62-39-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00")),
            ],
        );
        let inputs = &s["fairlight"]["inputs"];
        assert_eq!(
            inputs["4"]["sources"]["5521260141153198281"]["equalizer"]["bands"]["5"],
            json!({"enabled": true, "shapes": ["bell"], "shape": "low_shelf",
                   "frequency_ranges": [1], "frequency_range": 1, "frequency": 11159,
                   "gain": -7.27, "q": 6.29})
        );
        assert_eq!(
            inputs["1101"]["sources"]["3043462875041801726"]["dynamics"]["compressor"],
            json!({"enabled": false, "threshold": -3.75, "ratio": 11.42, "attack": 35.8,
                   "hold": 2720.08, "release": 3246.36})
        );
        assert_eq!(
            inputs["6"]["sources"]["3975437388773063523"]["dynamics"]["expander"],
            json!({"enabled": false, "gate": true, "threshold": -43.18, "range": 10.36,
                   "ratio": 2.53, "attack": 65.82, "hold": 3914.3, "release": 2800.02})
        );
        assert_eq!(
            inputs["2003"]["sources"]["-6943392084754388106"]["dynamics"]["limiter"],
            json!({"enabled": true, "threshold": -5.34, "attack": 18.27, "hold": 3546.99,
                   "release": 304.96})
        );
        let source = &inputs["1"]["sources"]["6820216881589062371"];
        assert_eq!(source["type"], "stereo");
        assert_eq!(source["max_frames_delay"], 232);
        assert_eq!(source["frames_delay"], 59);
        assert_eq!(source["stereo_simulation"], 63.41);
        assert_eq!(
            source["equalizer"],
            json!({"enabled": true, "gain": -4.44, "band_count": 124})
        );
        assert_eq!(source["dynamics"]["make_up_gain"], 6.21);
        assert_eq!(source["balance"], 71.84);
        let master = &s["fairlight"]["master"];
        assert_eq!(
            master["dynamics"]["compressor"],
            json!({"enabled": false, "threshold": -45.57, "ratio": 11.05, "attack": 74.49,
                   "hold": 939.98, "release": 1736.03})
        );
        assert_eq!(
            master["dynamics"]["limiter"],
            json!({"enabled": true, "threshold": -11.37, "attack": 9.23, "hold": 2330.46,
                   "release": 3787.93})
        );
        assert_eq!(master["dynamics"]["make_up_gain"], 19.27);
        assert_eq!(master["equalizer"]["bands"]["1"]["shape"], "low_pass");
        assert_eq!(
            master["equalizer"]["bands"]["1"]["shapes"],
            json!(["high_shelf"])
        );
        assert_eq!(master["equalizer"]["bands"]["1"]["q"], 8.75);
        assert_eq!(master["equalizer"]["enabled"], true);
        assert_eq!(master["equalizer"]["gain"], -15.42);
        assert_eq!(master["equalizer"]["bands"]["1"]["enabled"], true);
        assert_eq!(
            s["fairlight"]["monitor"],
            json!({"gain": -5.94, "master_gain": -33.26, "master_muted": true,
                   "talkback_gain": -19.57, "talkback_muted": true, "sidetone_gain": -36.15})
        );
        assert_eq!(s["fairlight"]["audio_follow_video"], true);
        assert_eq!(
            s["fairlight"]["solo"],
            json!({"enabled": true, "input": 1, "source": -65280})
        );
        assert_eq!(
            s["fairlight"]["routing"]["outputs"]["266744094"],
            json!({"id": 4070, "channel_pair": "unknown", "source": 368626668,
                   "external_port": "rj45", "internal_port": "audio_aux_out",
                   "name": "a919a693-a3c3-4b9"})
        );
        // The input the test switcher reported, in the 2.30 layout.
        let s = decoded(
            &mut m,
            &[cmd(
                b"FAIP",
                &[0, 1, 2, 0, 0, 0, 0, 0x20, 0, 7, 2, 5, 4, 0, 0, 0],
            )],
        );
        assert_eq!(
            s["fairlight"]["inputs"]["1"],
            json!({"type": "audio_in", "external_port": "xlr",
                   "configurations": ["mono", "stereo", "dual_mono"], "configuration": "stereo",
                   "input_levels": ["microphone", "pro_line"], "input_level": "pro_line"})
        );
        // A removed source leaves the state and can no longer be set.
        let mut fasd = vec![0u8; 16];
        fasd[1] = 1;
        fasd[8..16].copy_from_slice(&SOURCE);
        let s = decoded(&mut m, &[cmd(b"FASD", &fasd)]);
        assert_eq!(
            s["fairlight"]["inputs"]["1"]["sources"]["-65280"],
            Value::Null
        );
        assert!(matches!(
            refused(
                &mut m,
                "set_fairlight_source",
                json!({"input": 1, "source": -65280, "gain": 0.0})
            ),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn classic_state_from_the_libatem_samples() {
        let mut m = classic();
        let s = decoded(
            &mut m,
            &[
                cmd(b"AMmO", &hex("01-00-31-E9-00-01-00-0A-01-00-26-09")),
                cmd(b"AMHP", &hex("F4-61-77-62-06-39-F9-91")),
                cmd(b"AMPP", &[0, 0, 0, 0]),
                cmd(b"AMMO", &hex("2F-E4-E4-CA-01-00-00-F4")),
            ],
        );
        assert_eq!(
            s["audio"]["monitor"],
            json!({"enabled": true, "gain": -8.18, "mute": false, "solo": true,
                   "solo_input": 10, "dim": true, "dim_level": 97.37})
        );
        assert_eq!(s["audio"]["headphones"]["gain"], 5.62);
        assert_eq!(s["audio"]["audio_follow_video"], false);
        assert_eq!(s["audio"]["master"]["follow_fade_to_black"], true);
    }
}
