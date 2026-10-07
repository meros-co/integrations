//! Blackmagic camera control over SDI: what the switcher reports back as
//! CCdP, and the commands beyond iris, focus, zoom, gain, white balance and
//! shutter.
//!
//! Categories, parameters and data types from @atem-connection/camera-control
//! 0.4.0 (`ids.d.ts`, `commandSender/baseGenerator.js` and the state builders
//! in `stateBuilder/`), framing from Sofie `CameraControlCommand.ts`.

use serde_json::{json, Map, Value};

use super::{
    camera_body, i16_at, i32_at, invalid, one, opt_bool, opt_num, u16_at, Atem, CommandError, Out,
    Params, CC_BOOL, CC_FLOAT, CC_SINT16, CC_SINT32, CC_SINT8,
};

/// Sharpening levels (camera-control `state.d.ts` VideoSharpeningLevel).
pub(super) const SHARPENING: [&str; 4] = ["off", "low", "medium", "high"];

/// Colour wheels: name, parameter in category 8, and the range of red,
/// green, blue and luma (Blackmagic's camera control protocol, as the
/// cameras' manuals give it; 15.99 is the most 5.11 fixed point holds).
pub(super) const WHEELS: [(&str, u8, f64, f64); 4] = [
    ("lift", 0, -2.0, 2.0),
    ("gamma", 1, -4.0, 4.0),
    ("gain", 2, 0.0, 15.99),
    ("offset", 3, -8.0, 8.0),
];

/// A CCdP body's values as numbers: booleans and integers as they are,
/// floats from 5.11 fixed point. Counts of 8-, 16- and 32-bit values at 4,
/// 6 and 8, values from 16 (Sofie `CameraControlCommand.ts:177-249`).
fn values(b: &[u8]) -> Option<Vec<f64>> {
    let kind = b[3];
    let (count, width) = match kind {
        CC_BOOL | CC_SINT8 => (u16_at(b, 4) as usize, 1),
        CC_SINT16 | CC_FLOAT => (u16_at(b, 6) as usize, 2),
        CC_SINT32 => (u16_at(b, 8) as usize, 4),
        _ => return None,
    };
    if b.len() < 16 + count * width {
        return None;
    }
    Some(
        (0..count)
            .map(|i| {
                let at = 16 + i * width;
                match kind {
                    CC_BOOL => (b[at] != 0) as u8 as f64,
                    CC_SINT8 => b[at] as i8 as f64,
                    CC_SINT16 => i16_at(b, at) as f64,
                    CC_FLOAT => i16_at(b, at) as f64 / 2048.0,
                    _ => i32_at(b, at) as f64,
                }
            })
            .collect(),
    )
}

/// Four values as red, green, blue and luma.
fn rgby(v: &[f64]) -> Value {
    json!({"red": v[0], "green": v[1], "blue": v[2], "luma": v[3]})
}

impl Atem {
    /// Camera control: camera, category, parameter, type, the counts, the
    /// values from 16. Only what camera-control's state builders read is
    /// kept; other parameters are ignored.
    pub(super) fn decode_camera(&mut self, b: &[u8]) -> Option<Value> {
        if b.len() < 16 {
            return None;
        }
        let (camera, category, parameter, kind) = (b[0], b[1], b[2], b[3]);
        let v = values(b)?;
        let want = |t: u8, n: usize| (kind == t && v.len() >= n).then_some(());
        let field = match (category, parameter) {
            (0, 0) => {
                want(CC_FLOAT, 1)?;
                json!({"focus": v[0]})
            }
            (0, 2) => {
                want(CC_FLOAT, 1)?;
                json!({"iris_fstop": v[0]})
            }
            (0, 3) => {
                want(CC_FLOAT, 1)?;
                json!({"iris": v[0]})
            }
            (0, 6) => {
                want(CC_BOOL, 1)?;
                json!({"stabilisation": v[0] != 0.0})
            }
            (0, 9) => {
                want(CC_FLOAT, 1)?;
                json!({"zoom_speed": v[0]})
            }
            (1, 2) => {
                want(CC_SINT16, 2)?;
                json!({"white_balance": v[0] as i64, "tint": v[1] as i64})
            }
            (1, 5) => {
                want(CC_SINT32, 1)?;
                json!({"exposure_us": v[0] as i64})
            }
            (1, 8) => {
                want(CC_SINT8, 1)?;
                json!({"sharpening": super::named(&SHARPENING, v[0] as u8)})
            }
            (1, 13) => {
                want(CC_SINT8, 1)?;
                json!({"gain": v[0] as i64})
            }
            (1, 16) => {
                want(CC_FLOAT, 1)?;
                json!({"nd_filter": v[0]})
            }
            (3, 0) => {
                want(CC_SINT16, 1)?;
                json!({"overlays": v[0] != 0.0})
            }
            // Bits: 1 zebra, 2 focus assist, 4 false colour.
            (4, 1) => {
                want(CC_SINT16, 1)?;
                let bits = v[0] as i64;
                json!({"display_tools": {
                    "zebra": bits & 1 != 0,
                    "focus_assist": bits & 2 != 0,
                    "false_color": bits & 4 != 0,
                }})
            }
            // The bars show for this many seconds; camera-control reads
            // more than 1 as on.
            (4, 4) => {
                want(CC_SINT8, 1)?;
                json!({"color_bars": v[0] > 1.0})
            }
            (8, p @ 0..=3) => {
                want(CC_FLOAT, 4)?;
                let mut m = Map::new();
                m.insert(WHEELS[p as usize].0.into(), rgby(&v));
                json!({"color": m})
            }
            (8, 4) => {
                want(CC_FLOAT, 2)?;
                json!({"color": {"contrast": {"pivot": v[0], "adjust": v[1]}}})
            }
            (8, 5) => {
                want(CC_FLOAT, 1)?;
                json!({"color": {"luma_mix": v[0]}})
            }
            (8, 6) => {
                want(CC_FLOAT, 2)?;
                json!({"color": {"hue": v[0], "saturation": v[1]}})
            }
            _ => return None,
        };
        Some(json!({"cameras": {camera.to_string(): field}}))
    }

    pub(super) fn build_camera(
        &self,
        name: &str,
        params: &Params,
    ) -> Result<Option<Out>, CommandError> {
        let num = |field: &str| opt_num(params, field).unwrap_or(0.0);
        let fixed = |v: f64| (v * 2048.0).round() as i64;
        let cc = |category: u8, parameter: u8, kind: u8, values: &[i64]| {
            let cam = self.check_camera(name, params)?;
            one(
                b"CCmd",
                camera_body(cam, category, parameter, false, kind, values),
            )
        };
        match name {
            // Triggers carry a BOOL type and no values.
            "camera_auto_focus" => cc(0, 1, CC_BOOL, &[]),
            "camera_auto_iris" => cc(0, 5, CC_BOOL, &[]),
            "camera_auto_white_balance" => cc(1, 3, CC_BOOL, &[]),
            "camera_color_reset" => cc(8, 7, CC_BOOL, &[]),
            "camera_iris_fstop" => cc(0, 2, CC_FLOAT, &[fixed(num("fstop"))]),
            "camera_stabilisation" => cc(0, 6, CC_BOOL, &[super::flag(params, "enabled") as i64]),
            "camera_sharpening" => {
                let level = super::enum_param(params, "level", &SHARPENING)?
                    .ok_or_else(|| invalid("level is required".into()))?;
                cc(1, 8, CC_SINT8, &[level as i64])
            }
            "camera_nd_filter" => cc(1, 16, CC_FLOAT, &[fixed(num("stop"))]),
            // On is 30 (seconds of bars), as camera-control sends it.
            "camera_color_bars" => cc(
                4,
                4,
                CC_SINT8,
                &[if super::flag(params, "enabled") {
                    30
                } else {
                    0
                }],
            ),
            "camera_display_tools" => {
                let bits = [("zebra", 1), ("focus_assist", 2), ("false_color", 4)]
                    .iter()
                    .filter(|(f, _)| opt_bool(params, f) == Some(true))
                    .fold(0, |b, (_, v)| b | v);
                cc(4, 1, CC_SINT16, &[bits])
            }
            "camera_overlays" => cc(3, 0, CC_SINT16, &[super::flag(params, "enabled") as i64]),
            "camera_color_wheel" => {
                let wheel = super::opt_str(params, "wheel").unwrap_or("");
                let Some(&(_, parameter, min, max)) = WHEELS.iter().find(|w| w.0 == wheel) else {
                    return Err(invalid(format!("unknown wheel {wheel:?}")));
                };
                let mut v = Vec::new();
                for field in ["red", "green", "blue", "luma"] {
                    let x = num(field);
                    if !(min..=max).contains(&x) {
                        return Err(invalid(format!(
                            "{wheel} {field} is {x}; it takes {min} to {max}"
                        )));
                    }
                    v.push(fixed(x));
                }
                cc(8, parameter, CC_FLOAT, &v)
            }
            // Pivot first, then the adjustment (camera-control colorContrastAdjust).
            "camera_contrast" => cc(8, 4, CC_FLOAT, &[fixed(num("pivot")), fixed(num("adjust"))]),
            "camera_luma_mix" => cc(8, 5, CC_FLOAT, &[fixed(num("luma_mix"))]),
            "camera_hue_saturation" => cc(
                8,
                6,
                CC_FLOAT,
                &[fixed(num("hue")), fixed(num("saturation"))],
            ),
            // Transport mode: 2 records, 0 stops; five SINT8 values
            // (camera-control mediaTriggerSetRecording, mediaTriggerSetStopped).
            "camera_record" => {
                let mode = if super::flag(params, "recording") {
                    2
                } else {
                    0
                };
                cc(10, 1, CC_SINT8, &[mode, 0, 0, 0, 0])
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cmd, decoded, payload, ready, refused};
    use super::super::{command, CommandError};
    use serde_json::json;

    /// A CCmd header: camera 1, category, parameter, not relative, type,
    /// rsv, then the 8-, 16-, 32- and 64-bit counts at 6 to 12.
    fn head(category: u8, parameter: u8, kind: u8, counts: [u16; 4]) -> Vec<u8> {
        let mut h = vec![1, category, parameter, 0, kind, 0];
        for c in counts {
            h.extend_from_slice(&c.to_be_bytes());
        }
        h.extend_from_slice(&[0, 0]);
        h
    }

    #[test]
    fn camera_command_layouts() {
        let (mut m, _) = ready();
        let cam = |extra: serde_json::Value| {
            let mut p = json!({"camera": 1});
            p.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            p
        };
        // Triggers: a BOOL type and no values.
        assert_eq!(
            payload(&mut m, "camera_auto_focus", cam(json!({}))),
            command(b"CCmd", &head(0, 1, 0, [0; 4]))
        );
        assert_eq!(
            payload(&mut m, "camera_color_reset", cam(json!({}))),
            command(b"CCmd", &head(8, 7, 0, [0; 4]))
        );
        let mut b = head(0, 6, 0, [1, 0, 0, 0]);
        b.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            payload(
                &mut m,
                "camera_stabilisation",
                cam(json!({"enabled": true}))
            ),
            command(b"CCmd", &b)
        );
        let mut b = head(1, 8, 1, [1, 0, 0, 0]);
        b.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            payload(&mut m, "camera_sharpening", cam(json!({"level": "medium"}))),
            command(b"CCmd", &b)
        );
        let mut b = head(8, 0, 0x80, [0, 4, 0, 0]);
        b.extend_from_slice(&[0x04, 0x00, 0xFE, 0x00, 0, 0, 0x08, 0x00]);
        assert_eq!(
            payload(
                &mut m,
                "camera_color_wheel",
                cam(json!({"wheel": "lift", "red": 0.5, "green": -0.25, "blue": 0.0, "luma": 1.0})),
            ),
            command(b"CCmd", &b)
        );
        let mut b = head(8, 4, 0x80, [0, 2, 0, 0]);
        b.extend_from_slice(&[0x04, 0x00, 0x08, 0x00, 0, 0, 0, 0]);
        assert_eq!(
            payload(
                &mut m,
                "camera_contrast",
                cam(json!({"pivot": 0.5, "adjust": 1.0}))
            ),
            command(b"CCmd", &b)
        );
        let mut b = head(10, 1, 1, [5, 0, 0, 0]);
        b.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            payload(&mut m, "camera_record", cam(json!({"recording": true}))),
            command(b"CCmd", &b)
        );
        let mut b = head(4, 4, 1, [1, 0, 0, 0]);
        b.extend_from_slice(&[30, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            payload(&mut m, "camera_color_bars", cam(json!({"enabled": true}))),
            command(b"CCmd", &b)
        );
        let mut b = head(4, 1, 2, [0, 1, 0, 0]);
        b.extend_from_slice(&[0, 5, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            payload(
                &mut m,
                "camera_display_tools",
                cam(json!({"zebra": true, "false_color": true})),
            ),
            command(b"CCmd", &b)
        );
        // Each wheel has its own range.
        assert!(matches!(
            refused(
                &mut m,
                "camera_color_wheel",
                cam(json!({"wheel": "gamma", "red": 5.0, "green": 0.0, "blue": 0.0, "luma": 0.0})),
            ),
            CommandError::InvalidParams { .. }
        ));
    }

    /// A CCdP body for camera 1: category, parameter, type, the 8-, 16- and
    /// 32-bit counts at 4, 6 and 8, then the data from 16.
    fn ccdp(category: u8, parameter: u8, kind: u8, counts: [u16; 3], data: &[u8]) -> Vec<u8> {
        let mut b = vec![1, category, parameter, kind];
        for c in counts {
            b.extend_from_slice(&c.to_be_bytes());
        }
        b.resize(16, 0);
        b.extend_from_slice(data);
        b.resize(16 + data.len().div_ceil(8) * 8, 0);
        cmd(b"CCdP", &b)
    }

    #[test]
    fn camera_state() {
        let (mut m, _) = ready();
        let s = decoded(
            &mut m,
            &[
                ccdp(8, 0, 0x80, [0, 4, 0], &[0x04, 0, 0xFE, 0, 0, 0, 0x08, 0]),
                ccdp(8, 4, 0x80, [0, 2, 0], &[0x04, 0, 0x08, 0]),
                ccdp(8, 6, 0x80, [0, 2, 0], &[0xFC, 0, 0x0C, 0]),
                ccdp(1, 8, 1, [1, 0, 0], &[3]),
                ccdp(1, 16, 0x80, [0, 1, 0], &[0x10, 0]),
                ccdp(0, 6, 0, [1, 0, 0], &[1]),
                ccdp(0, 2, 0x80, [0, 1, 0], &[0x08, 0]),
                ccdp(4, 1, 2, [0, 1, 0], &[0, 3]),
                ccdp(4, 4, 1, [1, 0, 0], &[30]),
                ccdp(3, 0, 2, [0, 1, 0], &[0, 1]),
                ccdp(1, 13, 1, [1, 0, 0], &[12]),
            ],
        );
        let c = &s["cameras"]["1"];
        assert_eq!(
            c["color"]["lift"],
            json!({"red": 0.5, "green": -0.25, "blue": 0.0, "luma": 1.0})
        );
        assert_eq!(c["color"]["contrast"], json!({"pivot": 0.5, "adjust": 1.0}));
        assert_eq!(c["color"]["hue"], -0.5);
        assert_eq!(c["color"]["saturation"], 1.5);
        assert_eq!(c["sharpening"], "high");
        assert_eq!(c["nd_filter"], 2.0);
        assert_eq!(c["stabilisation"], true);
        assert_eq!(c["iris_fstop"], 1.0);
        assert_eq!(
            c["display_tools"],
            json!({"zebra": true, "focus_assist": true, "false_color": false})
        );
        assert_eq!(c["color_bars"], true);
        assert_eq!(c["overlays"], true);
        assert_eq!(c["gain"], 12);
    }
}
