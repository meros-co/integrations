//! Upstream keyer DVE, chroma and advanced chroma settings, flying-key
//! keyframes, and upstream and downstream key masks.
//!
//! Layouts from Sofie `commands/MixEffects/Key/` and
//! `commands/DownstreamKey/DownstreamKeyMaskCommand.ts`; fixed-point scaling
//! from Companion `actions/mixeffect/upstreamKeyerDVE.ts:211-330` and
//! `options/upstreamKeyer.ts:398-450`, cross-checked against the LibAtem
//! samples in Sofie `commands/__tests__/libatem-data.json`, which give each
//! field both as raw bytes and in the user's units.

use serde_json::{json, Map, Value};

use super::{fields, invalid, one, opt_str, unsupported, Atem, Body, CommandError, Out, Params, W};

/// Border bevel styles (Sofie `enums/index.ts:174-179`).
pub(super) const BEVELS: [&str; 4] = ["none", "in_out", "in", "out"];

/// Directions a flying key runs to infinity in (Sofie `enums/index.ts:224-235`).
pub(super) const DIRECTIONS: [&str; 10] = [
    "centre_of_key",
    "top_left",
    "top_centre",
    "top_right",
    "middle_left",
    "middle_centre",
    "middle_right",
    "bottom_left",
    "bottom_centre",
    "bottom_right",
];

/// DVE fields: name, mask bit and offset in CKDV, offset in KeDV, width,
/// scale. Size and position in thousandths, rotation and colours in tenths,
/// border widths in hundredths, softness, opacity and altitude as is.
const DVE: [(&str, u32, usize, usize, W, f64); 25] = [
    ("size_x", 0, 8, 4, W::U32, 1000.0),
    ("size_y", 1, 12, 8, W::U32, 1000.0),
    ("x", 2, 16, 12, W::I32, 1000.0),
    ("y", 3, 20, 16, W::I32, 1000.0),
    ("rotation", 4, 24, 20, W::I32, 10.0),
    ("border_outer_width", 8, 32, 28, W::U16, 100.0),
    ("border_inner_width", 9, 34, 30, W::U16, 100.0),
    ("border_outer_softness", 10, 36, 32, W::U8, 1.0),
    ("border_inner_softness", 11, 37, 33, W::U8, 1.0),
    ("border_bevel_softness", 12, 38, 34, W::U8, 1.0),
    ("border_bevel_position", 13, 39, 35, W::U8, 1.0),
    ("border_opacity", 14, 40, 36, W::U8, 1.0),
    ("border_hue", 15, 42, 38, W::U16, 10.0),
    ("border_saturation", 16, 44, 40, W::U16, 10.0),
    ("border_luma", 17, 46, 42, W::U16, 10.0),
    ("light_direction", 18, 48, 44, W::U16, 10.0),
    ("light_altitude", 19, 50, 46, W::U8, 1.0),
    ("mask_top", 21, 52, 48, W::I16, 1000.0),
    ("mask_bottom", 22, 54, 50, W::I16, 1000.0),
    ("mask_left", 23, 56, 52, W::I16, 1000.0),
    ("mask_right", 24, 58, 54, W::I16, 1000.0),
    ("rate", 25, 60, 56, W::U8, 1.0),
    // The flags: offsets in CKDV and KeDV.
    ("border_enabled", 5, 28, 24, W::U8, 0.0),
    ("shadow_enabled", 6, 29, 25, W::U8, 0.0),
    ("mask_enabled", 20, 51, 47, W::U8, 0.0),
];

/// Keyframe fields: name, mask bit and offset in CKFP, offset in KKFP, width,
/// scale (Sofie `MixEffectKeyFlyKeyframeCommand.ts:21-55, 104-170`).
const KEYFRAME: [(&str, u32, usize, usize, W, f64); 21] = [
    ("size_x", 0, 8, 4, W::U32, 1000.0),
    ("size_y", 1, 12, 8, W::U32, 1000.0),
    ("x", 2, 16, 12, W::I32, 1000.0),
    ("y", 3, 20, 16, W::I32, 1000.0),
    ("rotation", 4, 24, 20, W::I32, 10.0),
    ("border_outer_width", 5, 28, 24, W::U16, 100.0),
    ("border_inner_width", 6, 30, 26, W::U16, 100.0),
    ("border_outer_softness", 7, 32, 28, W::U8, 1.0),
    ("border_inner_softness", 8, 33, 29, W::U8, 1.0),
    ("border_bevel_softness", 9, 34, 30, W::U8, 1.0),
    ("border_bevel_position", 10, 35, 31, W::U8, 1.0),
    ("border_opacity", 11, 36, 32, W::U8, 1.0),
    ("border_hue", 12, 38, 34, W::U16, 10.0),
    ("border_saturation", 13, 40, 36, W::U16, 10.0),
    ("border_luma", 14, 42, 38, W::U16, 10.0),
    ("light_direction", 15, 44, 40, W::U16, 10.0),
    ("light_altitude", 16, 46, 42, W::U8, 1.0),
    ("mask_top", 17, 48, 44, W::I16, 1000.0),
    ("mask_bottom", 18, 50, 46, W::I16, 1000.0),
    ("mask_left", 19, 52, 48, W::I16, 1000.0),
    ("mask_right", 20, 54, 50, W::I16, 1000.0),
];

/// Chroma key: name, mask bit, offset in CKCk, offset in KeCk; tenths
/// (Sofie `MixEffectKeyChromaCommand.ts:5-36, 54-66`).
const CHROMA: [(&str, u32, usize, usize); 4] = [
    ("hue", 0, 4, 2),
    ("gain", 1, 6, 4),
    ("y_suppress", 2, 8, 6),
    ("lift", 3, 10, 8),
];

/// Advanced chroma key: name, mask bit, offset in CACK, offset in KACk,
/// width; tenths of a percent (Sofie
/// `MixEffectKeyAdvancedChromaPropertiesCommand.ts:5-50, 68-82`).
const ADVANCED: [(&str, u32, usize, usize, W); 11] = [
    ("foreground_level", 0, 4, 2, W::U16),
    ("background_level", 1, 6, 4, W::U16),
    ("key_edge", 2, 8, 6, W::U16),
    ("spill_suppression", 3, 10, 8, W::U16),
    ("flare_suppression", 4, 12, 10, W::U16),
    ("brightness", 5, 14, 12, W::I16),
    ("contrast", 6, 16, 14, W::I16),
    ("saturation", 7, 18, 16, W::U16),
    ("red", 8, 20, 18, W::I16),
    ("green", 9, 22, 20, W::I16),
    ("blue", 10, 24, 22, W::I16),
];

/// Advanced chroma sample: name, mask bit, offset in CACC, offset in KACC,
/// width, scale (Sofie `MixEffectKeyAdvancedChromaSampleCommand.ts:5-42,
/// 60-74`): the cursor in thousandths of the frame's units, its size in
/// hundredths, the sampled colour in ten-thousandths.
const SAMPLE: [(&str, u32, usize, usize, W, f64); 6] = [
    ("cursor_x", 2, 6, 4, W::I16, 1000.0),
    ("cursor_y", 3, 8, 6, W::I16, 1000.0),
    ("cursor_size", 4, 10, 8, W::U16, 100.0),
    ("y", 5, 12, 10, W::U16, 10_000.0),
    ("cb", 6, 14, 12, W::I16, 10_000.0),
    ("cr", 7, 16, 14, W::I16, 10_000.0),
];

/// A mask's four edges in thousandths: top and bottom -9 to 9, left and right
/// -16 to 16 on a 16:9 frame.
const MASK: [&str; 4] = ["top", "bottom", "left", "right"];

/// Flags in `bits` named by position.
fn flags(bits: u8, names: &[&'static str]) -> Vec<&'static str> {
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| bits & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect()
}

/// A mask read from `b` at `at`: enabled, then four i16 edges from `at + 1`
/// or, when `gap`, from `at + 2`.
pub(super) fn mask(b: &[u8], at: usize, gap: bool) -> Value {
    let first = at + 1 + gap as usize;
    let mut m = Map::new();
    m.insert("enabled".into(), json!(b[at] == 1));
    for (i, edge) in MASK.iter().enumerate() {
        m.insert(
            (*edge).into(),
            json!(super::i16_at(b, first + 2 * i) as f64 / 1000.0),
        );
    }
    Value::Object(m)
}

/// A mask command's body: mask byte, the index bytes, enabled, then the four
/// edges as i16 from 4 (CKMs and CDsM share the shape).
fn mask_body(params: &Params, index: &[u8], len: usize) -> Result<Vec<u8>, CommandError> {
    let mut body = Body::new(params, len);
    for (i, v) in index.iter().enumerate() {
        body.at(1 + i, *v);
    }
    body.flag("enabled", 0, 1 + index.len());
    for (i, edge) in MASK.iter().enumerate() {
        body.num(edge, 1 + i as u32, 4 + 2 * i, 1000.0, W::I16);
    }
    body.done(0, W::U8)
}

impl Atem {
    pub(super) fn decode_keyers(&mut self, name: &[u8; 4], b: &[u8]) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        let one = |i: u8| (i as u32 + 1).to_string();
        let usk = |b: &[u8], key: &str, v: Value| json!({"mes": {one(b[0]): {"usk": {one(b[1]): {key: v}}}}});
        match name {
            // Sofie `MixEffectKeyDVECommand.ts:100-140`.
            b"KeDV" => {
                need(57)?;
                let mut m = Map::new();
                for &(name, _, _, at, w, by) in &DVE {
                    let value = if by == 0.0 {
                        json!(b[at] == 1)
                    } else {
                        fields(b, &[(name, at, w, by)])[name].clone()
                    };
                    m.insert(name.into(), value);
                }
                m.insert("border_bevel".into(), json!(super::named(&BEVELS, b[26])));
                Some(usk(b, "dve", Value::Object(m)))
            }
            b"KeCk" => {
                need(11)?;
                let mut m = fields(b, &CHROMA.map(|(name, _, _, at)| (name, at, W::U16, 10.0)));
                m.insert("narrow".into(), json!(b[10] == 1));
                Some(usk(b, "chroma", Value::Object(m)))
            }
            b"KACk" => {
                need(24)?;
                let m = fields(b, &ADVANCED.map(|(name, _, _, at, w)| (name, at, w, 10.0)));
                Some(usk(b, "advanced_chroma", Value::Object(m)))
            }
            b"KACC" => {
                need(16)?;
                let mut m = fields(b, &SAMPLE.map(|(name, _, _, at, w, by)| (name, at, w, by)));
                m.insert("cursor_enabled".into(), json!(b[2] != 0));
                m.insert("preview".into(), json!(b[3] != 0));
                Some(usk(b, "chroma_sample", Value::Object(m)))
            }
            // M/E, keyer, keyframe (1 A, 2 B), then the keyframe.
            b"KKFP" => {
                need(52)?;
                let frame = match b[2] {
                    1 => "a",
                    2 => "b",
                    _ => return None,
                };
                let m = fields(
                    b,
                    &KEYFRAME.map(|(name, _, _, at, w, by)| (name, at, w, by)),
                );
                Some(usk(b, "keyframes", json!({frame: m})))
            }
            // M/E, keyer, A set, B set, 2 rsv, at keyframe bits (1 A, 2 B,
            // 4 infinite), infinite direction (Sofie
            // `MixEffectKeyFlyPropertiesGetCommand.ts:20-30`).
            b"KeFS" => {
                need(8)?;
                Some(usk(
                    b,
                    "fly",
                    json!({
                        "a_set": b[2] == 1,
                        "b_set": b[3] == 1,
                        "at_keyframe": flags(b[6], &["a", "b", "infinite"]),
                        "infinite_direction": super::named(&DIRECTIONS, b[7]),
                    }),
                ))
            }
            _ => None,
        }
    }

    /// The (M/E, keyer) of a keyer that has a DVE to use.
    fn check_dve_keyer(&self, name: &str, params: &Params) -> Result<(u8, u8), CommandError> {
        if self.topology.dves == 0 {
            return Err(unsupported(name, "this switcher (no DVE)"));
        }
        let (me, keyer) = self.check_keyer(params)?;
        if self.topology.usk_can_fly.get(&(me, keyer)) == Some(&false) {
            return Err(invalid(format!("keyer {} cannot fly", keyer + 1)));
        }
        Ok((me, keyer))
    }

    pub(super) fn build_keyers(
        &self,
        name: &str,
        params: &Params,
    ) -> Result<Option<Out>, CommandError> {
        match name {
            // u32 mask, M/E, keyer, then the DVE from 8 (Sofie
            // `MixEffectKeyDVECommand.ts:5-75`).
            "set_usk_dve" => {
                let (me, keyer) = self.check_dve_keyer(name, params)?;
                let mut body = Body::new(params, 64);
                body.at(4, me).at(5, keyer);
                for &(field, bit, at, _, w, by) in &DVE {
                    if by == 0.0 {
                        body.flag(field, bit, at);
                    } else {
                        body.num(field, bit, at, by, w);
                    }
                }
                body.choice("border_bevel", 7, 30, &BEVELS)?;
                one(b"CKDV", body.done(0, W::U32)?)
            }
            // u32 mask, M/E, keyer, keyframe (1 A, 2 B), then the keyframe.
            "set_usk_keyframe" => {
                let (me, keyer) = self.check_dve_keyer(name, params)?;
                let frame = keyframe(params)?;
                let mut body = Body::new(params, 56);
                body.at(4, me).at(5, keyer).at(6, frame);
                for &(field, bit, at, _, w, by) in &KEYFRAME {
                    body.num(field, bit, at, by, w);
                }
                one(b"CKFP", body.done(0, W::U32)?)
            }
            // M/E, keyer, keyframe, rsv: stores the keyer's current DVE
            // settings as the keyframe (Sofie `MixEffectKeyFlyKeyframeCommand.ts:174-196`;
            // OpenSwitcher SFKF).
            "store_usk_keyframe" => {
                let (me, keyer) = self.check_dve_keyer(name, params)?;
                one(b"SFKF", vec![me, keyer, keyframe(params)?, 0])
            }
            // Mask (2 with a direction), M/E, keyer, rsv, keyframe (1 A, 2 B,
            // 3 full, 4 infinite), direction, 2 rsv (Sofie
            // `MixEffectKeyRunToCommand.ts:6-30`; OpenSwitcher RFlK).
            "usk_fly_to" => {
                let (me, keyer) = self.check_dve_keyer(name, params)?;
                let to = match opt_str(params, "keyframe") {
                    Some("a") => 1,
                    Some("b") => 2,
                    Some("full") => 3,
                    Some("infinite") => 4,
                    other => return Err(invalid(format!("unknown keyframe {other:?}"))),
                };
                let direction = super::enum_param(params, "direction", &DIRECTIONS)?;
                if to == 4 && direction.is_none() {
                    return Err(invalid("running to infinite needs a direction".into()));
                }
                let mask = if to == 4 { 2 } else { 0 };
                let direction = if to == 4 { direction.unwrap_or(0) } else { 0 };
                one(b"RFlK", vec![mask, me, keyer, 0, to, direction, 0, 0])
            }
            // Mask, M/E, keyer, rsv, then the four values in tenths, narrow.
            "set_usk_chroma" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut body = Body::new(params, 16);
                body.at(1, me).at(2, keyer);
                for &(field, bit, at, _) in &CHROMA {
                    body.num(field, bit, at, 10.0, W::U16);
                }
                body.flag("narrow", 4, 12);
                one(b"CKCk", body.done(0, W::U8)?)
            }
            "set_usk_advanced_chroma" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut body = Body::new(params, 28);
                body.at(2, me).at(3, keyer);
                for &(field, bit, at, _, w) in &ADVANCED {
                    body.num(field, bit, at, 10.0, w);
                }
                one(b"CACK", body.done(0, W::U16)?)
            }
            "set_usk_chroma_sample" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut body = Body::new(params, 20);
                body.at(1, me).at(2, keyer);
                body.flag("cursor_enabled", 0, 3).flag("preview", 1, 4);
                for &(field, bit, at, _, w, by) in &SAMPLE {
                    body.num(field, bit, at, by, w);
                }
                one(b"CACC", body.done(0, W::U8)?)
            }
            // M/E, keyer, rsv, bits: 0 key adjustments, 1 chroma
            // correction, 2 colour adjustments (Sofie
            // `MixEffectKeyAdvancedChromaSampleResetCommand.ts:20-40`).
            "reset_usk_advanced_chroma" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut bits = 0u8;
                for (i, part) in ["key_adjustments", "chroma_correction", "color_adjustments"]
                    .iter()
                    .enumerate()
                {
                    if super::opt_bool(params, part) == Some(true) {
                        bits |= 1 << i;
                    }
                }
                if bits == 0 {
                    return Err(invalid("give at least one part to reset".into()));
                }
                one(b"RACK", vec![me, keyer, 0, bits])
            }
            // Mask, M/E, keyer, enabled, i16 top, bottom, left, right
            // (Sofie `MixEffectKeyMaskSetCommand.ts:5-35`).
            "set_usk_mask" => {
                let (me, keyer) = self.check_keyer(params)?;
                one(b"CKMs", mask_body_keyer(params, me, keyer)?)
            }
            // Mask, DSK, enabled, rsv, i16 top, bottom, left, right (Sofie
            // `DownstreamKey/DownstreamKeyMaskCommand.ts:5-35`).
            "set_dsk_mask" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                one(b"CDsM", mask_body(params, &[dsk], 12)?)
            }
            _ => Ok(None),
        }
    }
}

/// CKMs: the enabled flag at 3, right after the M/E and keyer.
fn mask_body_keyer(params: &Params, me: u8, keyer: u8) -> Result<Vec<u8>, CommandError> {
    mask_body(params, &[me, keyer], 12)
}

fn keyframe(params: &Params) -> Result<u8, CommandError> {
    match opt_str(params, "keyframe") {
        Some("a") => Ok(1),
        Some("b") => Ok(2),
        other => Err(invalid(format!("unknown keyframe {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cmd, decoded, dump, hex, payload, ready, ready_with, refused};
    use super::super::{command, Atem, CommandError};
    use serde_json::{json, Value};

    #[test]
    fn dve_settings_layout() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_usk_dve",
            json!({"me": 1, "keyer": 1, "size_x": 0.5, "size_y": 0.5, "x": -8.0, "y": 4.5,
                   "rotation": 90.0, "border_enabled": true, "border_bevel": "in",
                   "mask_top": 1.5, "rate": 30}),
        );
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(&[0x02, 0x20, 0x00, 0xBF]);
        b[8..12].copy_from_slice(&500u32.to_be_bytes());
        b[12..16].copy_from_slice(&500u32.to_be_bytes());
        b[16..20].copy_from_slice(&(-8000i32).to_be_bytes());
        b[20..24].copy_from_slice(&4500i32.to_be_bytes());
        b[24..28].copy_from_slice(&900i32.to_be_bytes());
        b[28] = 1;
        b[30] = 2;
        b[52..54].copy_from_slice(&1500i16.to_be_bytes());
        b[60] = 30;
        assert_eq!(p, command(b"CKDV", &b));
    }

    #[test]
    fn keyframe_and_fly_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_usk_keyframe",
            json!({"me": 1, "keyer": 1, "keyframe": "a", "x": 1.0, "mask_right": -2.5}),
        );
        let mut b = vec![0u8; 56];
        b[..4].copy_from_slice(&[0, 0x10, 0, 0x04]);
        b[6] = 1;
        b[16..20].copy_from_slice(&1000i32.to_be_bytes());
        b[54..56].copy_from_slice(&(-2500i16).to_be_bytes());
        assert_eq!(p, command(b"CKFP", &b));
        let p = payload(
            &mut m,
            "store_usk_keyframe",
            json!({"me": 1, "keyer": 1, "keyframe": "b"}),
        );
        assert_eq!(p, command(b"SFKF", &[0, 0, 2, 0]));
        let p = payload(
            &mut m,
            "usk_fly_to",
            json!({"me": 1, "keyer": 1, "keyframe": "infinite", "direction": "top_right"}),
        );
        assert_eq!(p, command(b"RFlK", &[2, 0, 0, 0, 4, 3, 0, 0]));
        let p = payload(
            &mut m,
            "usk_fly_to",
            json!({"me": 1, "keyer": 1, "keyframe": "full"}),
        );
        assert_eq!(p, command(b"RFlK", &[0, 0, 0, 0, 3, 0, 0, 0]));
    }

    #[test]
    fn chroma_and_mask_layouts() {
        let (mut m, _) = ready();
        // Values from the LibAtem samples: hue 0x0733, saturation 0x0705,
        // blue 0xFD99, the sample cursor, mask top 0x06C0 and right 0xE068,
        // DSK mask bottom 0x11E9.
        let p = payload(
            &mut m,
            "set_usk_chroma",
            json!({"me": 1, "keyer": 1, "hue": 184.3, "gain": 39.8}),
        );
        assert_eq!(
            p,
            command(
                b"CKCk",
                &[3, 0, 0, 0, 0x07, 0x33, 0x01, 0x8E, 0, 0, 0, 0, 0, 0, 0, 0]
            )
        );
        let p = payload(
            &mut m,
            "set_usk_advanced_chroma",
            json!({"me": 1, "keyer": 1, "saturation": 179.7, "blue": -61.5}),
        );
        let mut b = vec![0u8; 28];
        b[..2].copy_from_slice(&[0x04, 0x80]);
        b[18..20].copy_from_slice(&[0x07, 0x05]);
        b[24..26].copy_from_slice(&[0xFD, 0x99]);
        assert_eq!(p, command(b"CACK", &b));
        let p = payload(
            &mut m,
            "set_usk_chroma_sample",
            json!({"me": 1, "keyer": 1, "cursor_enabled": true, "cursor_x": 5.109,
                   "cursor_y": -6.502, "cursor_size": 73.61, "y": 0.7437}),
        );
        assert_eq!(
            p,
            command(
                b"CACC",
                &hex("3D-00-00-01-00-00-13-F5-E6-9A-1C-C1-1D-0D-00-00-00-00-00-00")
            )
        );
        let p = payload(
            &mut m,
            "reset_usk_advanced_chroma",
            json!({"me": 1, "keyer": 1, "chroma_correction": true}),
        );
        assert_eq!(p, command(b"RACK", &[0, 0, 0, 2]));
        let p = payload(
            &mut m,
            "set_usk_mask",
            json!({"me": 1, "keyer": 1, "enabled": true, "top": 1.728, "right": -8.088}),
        );
        assert_eq!(
            p,
            command(b"CKMs", &hex("13-00-00-01-06-C0-00-00-00-00-E0-68"))
        );
        let p = payload(&mut m, "set_dsk_mask", json!({"dsk": 1, "bottom": 4.585}));
        assert_eq!(
            p,
            command(b"CDsM", &hex("04-00-00-00-00-00-11-E9-00-00-00-00"))
        );
    }

    #[test]
    fn keyer_commands_are_checked() {
        let (mut m, _) = ready();
        let bad = |m: &mut Atem, name: &str, params: Value| {
            assert!(
                matches!(refused(m, name, params), CommandError::InvalidParams { .. }),
                "{name}"
            )
        };
        bad(
            &mut m,
            "set_usk_dve",
            json!({"me": 1, "keyer": 2, "rate": 10}),
        );
        bad(&mut m, "set_usk_dve", json!({"me": 1, "keyer": 1}));
        bad(
            &mut m,
            "usk_fly_to",
            json!({"me": 1, "keyer": 1, "keyframe": "infinite"}),
        );
        bad(
            &mut m,
            "reset_usk_advanced_chroma",
            json!({"me": 1, "keyer": 1}),
        );
        bad(&mut m, "set_dsk_mask", json!({"dsk": 2, "enabled": true}));

        // No DVE: the DVE and flying-key commands are unsupported.
        let mut d = dump();
        let mut top = vec![0u8; 24];
        top[0] = 1;
        top[1] = 3;
        top[2] = 1;
        top[3] = 1;
        top[5] = 2;
        d[2] = cmd(b"_top", &top);
        let mut m = ready_with(d);
        assert!(matches!(
            refused(
                &mut m,
                "set_usk_dve",
                json!({"me": 1, "keyer": 1, "rate": 10})
            ),
            CommandError::UnsupportedForModel { .. }
        ));
    }

    #[test]
    fn keyer_state_from_the_libatem_samples() {
        let (mut m, _) = ready();
        let s = decoded(
            &mut m,
            &[
                cmd(b"KeDV", &hex("00-01-00-00-00-00-CF-59-00-01-44-A1-00-04-24-4F-00-07-22-46-FF-FD-B7-6E-01-01-00-00-02-60-05-AB-51-37-18-4B-1A-00-03-31-01-A4-01-4E-09-EB-3D-00-18-CB-2F-26-89-39-28-EE-1C-00-00-00")),
                cmd(b"KeCk", &hex("00-01-07-FF-00-92-01-19-01-7E-01-00")),
                cmd(b"KACk", &hex("00-00-00-FB-01-E5-00-D1-02-71-03-A6-FC-98-FE-31-00-88-FE-5A-00-52-FE-E1")),
                cmd(b"KACC", &hex("03-01-00-00-22-EA-06-D6-26-B0-1B-FB-08-FB-10-FC")),
                cmd(b"KKFP", &hex("01-00-01-00-01-EC-63-5C-00-FC-B9-C8-FF-88-D3-25-01-32-A5-34-00-01-BE-3C-E2-D7-C0-E9-CF-69-4F-B4-00-00-A9-06-FE-26-A0-BB-0F-64-6F-00-1E-92-02-C5-15-63-FE-24")),
                cmd(b"KeFS", &[0, 0, 1, 0, 0, 0, 4, 3]),
                cmd(b"KeBP", &hex("01-02-01-00-00-01-1F-42-1F-47-01-00-E3-C1-E2-8C-0D-6E-27-66")),
                cmd(b"DskP", &hex("00-00-20-01-00-D0-00-2E-01-01-F3-91-11-2B-D8-8F-00-ED-00-00")),
            ],
        );
        let dve = &s["mes"]["1"]["usk"]["2"]["dve"];
        assert_eq!(dve["size_x"], 53.081);
        assert_eq!(dve["x"], 271.439);
        assert_eq!(dve["rotation"], -14965.0);
        assert_eq!(dve["border_enabled"], true);
        assert_eq!(dve["shadow_enabled"], true);
        assert_eq!(dve["border_bevel"], "none");
        assert_eq!(dve["border_outer_width"], 6.08);
        assert_eq!(dve["border_outer_softness"], 81);
        assert_eq!(dve["light_direction"], 253.9);
        assert_eq!(dve["light_altitude"], 61);
        assert_eq!(dve["mask_enabled"], false);
        assert_eq!(dve["mask_top"], 6.347);
        assert_eq!(dve["rate"], 28);
        assert_eq!(
            s["mes"]["1"]["usk"]["2"]["chroma"],
            json!({"hue": 204.7, "gain": 14.6, "y_suppress": 28.1, "lift": 38.2, "narrow": true})
        );
        let adv = &s["mes"]["1"]["usk"]["1"]["advanced_chroma"];
        assert_eq!(adv["foreground_level"], 25.1);
        assert_eq!(adv["brightness"], -87.2);
        assert_eq!(adv["saturation"], 13.6);
        assert_eq!(adv["blue"], -28.7);
        let sample = &s["mes"]["4"]["usk"]["2"]["chroma_sample"];
        assert_eq!(sample["cursor_x"], 8.938);
        assert_eq!(sample["cursor_size"], 99.04);
        assert_eq!(sample["y"], 0.7163);
        assert_eq!(sample["cursor_enabled"], false);
        let kf = &s["mes"]["2"]["usk"]["1"]["keyframes"]["a"];
        assert_eq!(kf["mask_top"], 7.826);
        assert_eq!(kf["light_altitude"], 111);
        assert_eq!(
            s["mes"]["1"]["usk"]["1"]["fly"],
            json!({"a_set": true, "b_set": false, "at_keyframe": ["infinite"],
                   "infinite_direction": "top_right"})
        );
        assert_eq!(
            s["mes"]["2"]["usk"]["3"]["mask"],
            json!({"enabled": true, "top": -7.231, "bottom": -7.54, "left": 3.438, "right": 10.086})
        );
        assert_eq!(
            s["dsks"]["1"]["mask"],
            json!({"enabled": true, "top": -3.183, "bottom": 4.395, "left": -10.097, "right": 0.237})
        );
    }
}
