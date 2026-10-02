//! Template rendering: SPEC.md §4, "Templates" and "Formatting directives".
//!
//! This is the only implementation of those rules. Every rendering decision a
//! language could make differently is made explicitly here: integer and float
//! formatting, rounding, boolean words and escaping.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::catalog::{ParamSpec, ParamType, Params};
use std::collections::BTreeMap;

fn placeholder() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A name is an identifier, so a literal brace in a JSON body, such as
    // {"status":"toggle"}, is never taken for a placeholder.
    RE.get_or_init(|| Regex::new(r#"\{([A-Za-z0-9_][A-Za-z0-9_.]*)((?::[^{}:"\s]+)*)\}"#).unwrap())
}

/// Where values come from: a command's parameters and the device's settings,
/// both already validated with defaults applied, and the spec's conversions.
pub(crate) struct Values<'a> {
    pub(crate) params: &'a Params,
    pub(crate) param_specs: &'a BTreeMap<String, ParamSpec>,
    pub(crate) settings: &'a Params,
    pub(crate) setting_specs: &'a BTreeMap<String, ParamSpec>,
    pub(crate) conversions: &'a Conversions,
}

/// A spec's named conversions: SPEC.md §4, "Conversions".
pub(crate) type Conversions = BTreeMap<String, Conversion>;

/// A piecewise-linear conversion between a wire value and the value an
/// operator uses (an X32 fader position and its dB), given as points joined
/// by straight lines. Both columns are strictly monotonic, so it inverts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Conversion {
    /// (wire, value), wire ascending.
    by_wire: Vec<(f64, f64)>,
    /// (value, wire), value ascending.
    by_value: Vec<(f64, f64)>,
}

impl Conversion {
    fn parse(name: &str, v: &Value) -> Result<Conversion, String> {
        let points = v
            .get("points")
            .and_then(Value::as_array)
            .ok_or(format!("conversion '{name}' needs points"))?;
        let mut by_wire = Vec::new();
        for p in points {
            let pair = p.as_array().filter(|a| a.len() == 2);
            match pair.and_then(|a| Some((a[0].as_f64()?, a[1].as_f64()?))) {
                Some(pair) => by_wire.push(pair),
                None => return Err(format!("conversion '{name}': a point is [wire, value]")),
            }
        }
        if by_wire.len() < 2 {
            return Err(format!("conversion '{name}' needs at least two points"));
        }
        fn rising(a: &[(f64, f64)]) -> bool {
            a.windows(2).all(|w| w[0].0 < w[1].0)
        }
        if !rising(&by_wire) {
            return Err(format!("conversion '{name}': wire values must rise"));
        }
        let mut by_value: Vec<(f64, f64)> = by_wire.iter().map(|&(w, v)| (v, w)).collect();
        if !rising(&by_value) {
            by_value.reverse();
            if !rising(&by_value) {
                return Err(format!(
                    "conversion '{name}': values must rise or fall with the wire"
                ));
            }
        }
        Ok(Conversion { by_wire, by_value })
    }

    /// The wire value for an operator value; `None` outside the points.
    pub(crate) fn value_to_wire(&self, value: f64) -> Option<f64> {
        interpolate(&self.by_value, value)
    }

    /// The operator value for a wire value; `None` outside the points.
    pub(crate) fn wire_to_value(&self, wire: f64) -> Option<f64> {
        interpolate(&self.by_wire, wire)
    }
}

/// Linear interpolation over points sorted by x. An x on a point gives that
/// point's y exactly; an x outside the points gives nothing.
fn interpolate(points: &[(f64, f64)], x: f64) -> Option<f64> {
    if let Some(&(_, y)) = points.iter().find(|(px, _)| *px == x) {
        return Some(y);
    }
    let i = points.windows(2).position(|w| w[0].0 < x && x < w[1].0)?;
    let ((x0, y0), (x1, y1)) = (points[i], points[i + 1]);
    Some(y0 + (x - x0) * (y1 - y0) / (x1 - x0))
}

/// The spec's `conversions` section.
pub(crate) fn conversions(spec: Option<&Value>) -> Result<Conversions, String> {
    let mut out = Conversions::new();
    for (name, v) in spec.and_then(Value::as_object).into_iter().flatten() {
        out.insert(name.clone(), Conversion::parse(name, v)?);
    }
    Ok(out)
}

/// A `to.<name>` or `from.<name>` directive: the conversion and its direction.
fn conversion_directive(d: &str) -> Option<(&str, bool)> {
    d.strip_prefix("to.")
        .map(|n| (n, true))
        .or_else(|| d.strip_prefix("from.").map(|n| (n, false)))
}

fn apply_conversion(x: f64, d: &str, values: &Values) -> Result<f64, String> {
    let (name, to_wire) = conversion_directive(d).ok_or(format!("bad directive ':{d}'"))?;
    let c = values
        .conversions
        .get(name)
        .ok_or(format!("conversion '{name}' is not declared"))?;
    let out = if to_wire {
        c.value_to_wire(x)
    } else {
        c.wire_to_value(x)
    };
    out.ok_or(format!("{x} is outside conversion '{name}'"))
}

impl Values<'_> {
    fn lookup(&self, name: &str) -> Result<(&Value, ParamType), String> {
        let (values, specs, key) = match name.strip_prefix("settings.") {
            Some(key) => (self.settings, self.setting_specs, key),
            None => (self.params, self.param_specs, name),
        };
        let spec = specs
            .get(key)
            .ok_or_else(|| format!("'{name}' is not declared"))?;
        let value = values
            .get(key)
            .ok_or_else(|| format!("'{name}' has no value"))?;
        Ok((value, spec.kind))
    }
}

/// Render `template`, passing each substituted value through `escape`. Literal
/// template text is never escaped.
pub(crate) fn render(
    template: &str,
    values: &Values,
    escape: fn(&str) -> String,
) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut last = 0;
    for caps in placeholder().captures_iter(template) {
        let whole = caps.get(0).unwrap();
        out.push_str(&template[last..whole.start()]);
        let name = caps.get(1).unwrap().as_str();
        let directives: Vec<&str> = caps[2].split(':').filter(|d| !d.is_empty()).collect();
        let text = render_one(name, &directives, values)?;
        out.push_str(&escape(&text));
        last = whole.end();
    }
    out.push_str(&template[last..]);
    Ok(out)
}

/// If `template` is exactly one placeholder with no directives, the raw value:
/// used for OSC arguments, where a number is sent as a number.
pub(crate) fn sole_value<'a>(template: &str, values: &'a Values) -> Option<(&'a Value, ParamType)> {
    let caps = placeholder().captures(template)?;
    if caps.get(0)?.as_str() != template || !caps[2].is_empty() {
        return None;
    }
    values.lookup(&caps[1]).ok()
}

/// If `template` is exactly one numeric placeholder whose directives are all
/// conversions, the converted number: an OSC float argument or a state value
/// takes it as a number, with no text form involved.
pub(crate) fn sole_converted(template: &str, values: &Values) -> Option<Result<f64, String>> {
    let caps = placeholder().captures(template)?;
    if caps.get(0)?.as_str() != template || caps[2].is_empty() {
        return None;
    }
    let directives: Vec<&str> = caps[2].split(':').filter(|d| !d.is_empty()).collect();
    if !directives.iter().all(|d| conversion_directive(d).is_some()) {
        return None;
    }
    let (value, kind) = match values.lookup(&caps[1]) {
        Ok(v) => v,
        Err(e) => return Some(Err(e)),
    };
    if !matches!(kind, ParamType::Int | ParamType::Float) {
        return Some(Err(format!(
            "a conversion needs a number, not '{}'",
            &caps[1]
        )));
    }
    let mut x = value.as_f64()?;
    for d in directives {
        x = match apply_conversion(x, d, values) {
            Ok(x) => x,
            Err(e) => return Some(Err(e)),
        };
    }
    Some(Ok(x))
}

fn render_one(name: &str, directives: &[&str], values: &Values) -> Result<String, String> {
    let (value, kind) = values.lookup(name)?;

    // Conversions come first and produce a float, which then needs `.Nf`.
    let converted = directives
        .iter()
        .take_while(|d| conversion_directive(d).is_some())
        .count();
    if converted > 0 {
        if !matches!(kind, ParamType::Int | ParamType::Float) {
            return Err(format!("a conversion needs a number, not '{name}'"));
        }
        let mut x = value
            .as_f64()
            .ok_or_else(|| format!("'{name}' is not a number"))?;
        for d in &directives[..converted] {
            x = apply_conversion(x, d, values)?;
        }
        let places = match &directives[converted..] {
            [d] => d.strip_prefix('.').and_then(|r| r.strip_suffix('f')),
            _ => None,
        }
        .ok_or_else(|| format!("converted '{name}' rendered as text needs one ':.Nf'"))?;
        let places: u32 = places
            .parse()
            .map_err(|_| format!("bad directive on '{name}'"))?;
        return Ok(fixed(x, places));
    }

    match kind {
        ParamType::Int => {
            let mut n = value
                .as_i64()
                .ok_or_else(|| format!("'{name}' is not an integer"))?;
            let mut width: Option<usize> = None;
            let mut signed = false;
            for d in directives {
                if let Some(offset) = parse_offset(d) {
                    n += offset;
                } else if let Some(w) = d.strip_prefix('0').and_then(|r| r.strip_suffix('d')) {
                    width = Some(w.parse().map_err(|_| format!("bad directive ':{d}'"))?);
                } else if *d == "signed" {
                    signed = true;
                } else {
                    return Err(format!("directive ':{d}' does not apply to an integer"));
                }
            }
            let digits = n.unsigned_abs().to_string();
            let digits = match width {
                Some(w) if digits.len() < w => format!("{}{digits}", "0".repeat(w - digits.len())),
                _ => digits,
            };
            let sign = if n < 0 {
                "-"
            } else if signed {
                "+"
            } else {
                ""
            };
            Ok(format!("{sign}{digits}"))
        }
        ParamType::Float => {
            let x = value
                .as_f64()
                .ok_or_else(|| format!("'{name}' is not a number"))?;
            let places = directives
                .iter()
                .find_map(|d| d.strip_prefix('.').and_then(|r| r.strip_suffix('f')))
                .ok_or_else(|| format!("float '{name}' rendered as text needs ':.Nf'"))?;
            let places: u32 = places
                .parse()
                .map_err(|_| format!("bad directive on '{name}'"))?;
            Ok(fixed(x, places))
        }
        ParamType::Bool => {
            let b = value
                .as_bool()
                .ok_or_else(|| format!("'{name}' is not a boolean"))?;
            let text = match directives {
                [] => {
                    if b {
                        "true"
                    } else {
                        "false"
                    }
                }
                ["on_off"] => {
                    if b {
                        "ON"
                    } else {
                        "OFF"
                    }
                }
                ["bool01"] => {
                    if b {
                        "1"
                    } else {
                        "0"
                    }
                }
                ["bool10"] => {
                    if b {
                        "0"
                    } else {
                        "1"
                    }
                }
                _ => return Err(format!("bad directives on boolean '{name}'")),
            };
            Ok(text.into())
        }
        ParamType::Json => {
            if !directives.is_empty() {
                return Err(format!("directives do not apply to JSON '{name}'"));
            }
            Ok(value.to_string())
        }
        ParamType::Enum | ParamType::String => {
            let mut s = value
                .as_str()
                .ok_or_else(|| format!("'{name}' is not a string"))?
                .to_string();
            for d in directives {
                s = match *d {
                    "upper" => s.to_uppercase(),
                    "lower" => s.to_lowercase(),
                    // A JSON string literal, quotes included, for request bodies.
                    "json" => Value::String(s).to_string(),
                    // Percent-encoded, for a raw_query.
                    "url" => percent_encode(&s),
                    _ => return Err(format!("directive ':{d}' does not apply to a string")),
                };
            }
            Ok(s)
        }
    }
}

fn parse_offset(d: &str) -> Option<i64> {
    let (sign, digits) = match d.as_bytes().first()? {
        b'+' => (1, &d[1..]),
        b'-' => (-1, &d[1..]),
        _ => return None,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok().map(|n| sign * n)
}

/// Fixed decimals, rounded half away from zero, formatted from an integer so
/// no language's float formatting is involved.
pub(crate) fn fixed(x: f64, places: u32) -> String {
    let scaled = (x * 10f64.powi(places as i32)).round() as i128;
    let negative = scaled < 0;
    let digits = scaled.unsigned_abs().to_string();
    let places = places as usize;
    let digits = if digits.len() <= places {
        format!("{}{digits}", "0".repeat(places + 1 - digits.len()))
    } else {
        digits
    };
    let (int, frac) = digits.split_at(digits.len() - places);
    let sign = if negative { "-" } else { "" };
    if places == 0 {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac}")
    }
}

pub(crate) fn no_escape(s: &str) -> String {
    s.to_string()
}

/// RFC 3986 percent-encoding, keeping only unreserved characters.
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn values(params: Value, specs: &str) -> (Params, BTreeMap<String, ParamSpec>) {
        (
            params.as_object().unwrap().clone(),
            serde_yaml::from_str(specs).unwrap(),
        )
    }

    fn go(template: &str, params: Value, specs: &str) -> Result<String, String> {
        let (p, s) = values(params, specs);
        let empty = Params::new();
        let empty_specs = BTreeMap::new();
        let conversions = conversions(Some(&json!({
            "fader": {"points": [[0.0, -90.0], [0.0625, -60.0], [0.25, -30.0], [0.5, -10.0], [1.0, 10.0]]},
        })))
        .unwrap();
        let v = Values {
            params: &p,
            param_specs: &s,
            settings: &empty,
            setting_specs: &empty_specs,
            conversions: &conversions,
        };
        render(template, &v, no_escape)
    }

    #[test]
    fn conversions_both_ways() {
        let c = conversions(Some(&json!({
            "fader": {"points": [[0.0, -90.0], [0.0625, -60.0], [0.25, -30.0], [0.5, -10.0], [1.0, 10.0]]},
        })))
        .unwrap();
        let f = &c["fader"];
        // Maillot p.128: 0 dB is 0.75, +10 is 1.0, the segment ends exact.
        assert_eq!(f.value_to_wire(0.0), Some(0.75));
        assert_eq!(f.value_to_wire(10.0), Some(1.0));
        assert_eq!(f.value_to_wire(-30.0), Some(0.25));
        assert_eq!(f.value_to_wire(-75.0), Some(0.03125));
        assert_eq!(f.wire_to_value(0.75), Some(0.0));
        assert_eq!(f.wire_to_value(0.375), Some(-20.0));
        assert_eq!(f.wire_to_value(0.0), Some(-90.0));
        // Outside the points there is no value: never clamped.
        assert_eq!(f.value_to_wire(10.5), None);
        assert_eq!(f.wire_to_value(1.01), None);
        let s = "db: { type: float }";
        assert_eq!(
            go("{db:to.fader:.4f}", json!({"db": -20.0}), s).unwrap(),
            "0.3750"
        );
        assert!(go("{db:to.fader}", json!({"db": -20.0}), s).is_err());
        assert!(go("{db:to.fader:.1f}", json!({"db": 11.0}), s).is_err());
        assert!(go("{db:to.nope:.1f}", json!({"db": 1.0}), s).is_err());
        // A falling table inverts too.
        let c = conversions(Some(&json!({"att": {"points": [[0, 0], [100, -50]]}}))).unwrap();
        assert_eq!(c["att"].value_to_wire(-25.0), Some(50.0));
        assert!(conversions(Some(&json!({"bad": {"points": [[0, 0], [1, 1], [2, 0]]}}))).is_err());
    }

    #[test]
    fn integers_offsets_padding_and_sign() {
        let s = "n: { type: int }";
        assert_eq!(go("{n:02d}", json!({"n": 7}), s).unwrap(), "07");
        assert_eq!(go("{n:-1:02d}", json!({"n": 1}), s).unwrap(), "00");
        assert_eq!(go("{n:signed}", json!({"n": 7}), s).unwrap(), "+7");
        assert_eq!(go("{n:signed}", json!({"n": -7}), s).unwrap(), "-7");
        assert_eq!(go("{n:03d}", json!({"n": -7}), s).unwrap(), "-007");
        assert_eq!(go("#R{n:-1:02d}", json!({"n": 100}), s).unwrap(), "#R99");
    }

    #[test]
    fn booleans() {
        let s = "b: { type: bool }";
        assert_eq!(go("{b}", json!({"b": true}), s).unwrap(), "true");
        assert_eq!(go("{b:on_off}", json!({"b": false}), s).unwrap(), "OFF");
        assert_eq!(go("{b:bool01}", json!({"b": true}), s).unwrap(), "1");
        assert_eq!(go("{b:bool10}", json!({"b": true}), s).unwrap(), "0");
    }

    #[test]
    fn floats_round_half_away_from_zero() {
        assert_eq!(fixed(0.125, 2), "0.13");
        assert_eq!(fixed(-0.125, 2), "-0.13");
        assert_eq!(fixed(0.75, 2), "0.75");
        assert_eq!(fixed(1.0, 1), "1.0");
        assert_eq!(fixed(0.05, 1), "0.1");
        assert_eq!(fixed(2.5, 0), "3");
        let s = "x: { type: float }";
        assert_eq!(go("{x:.2f}", json!({"x": 0.5}), s).unwrap(), "0.50");
        assert!(go("{x}", json!({"x": 0.5}), s).is_err());
    }

    #[test]
    fn strings_and_escaping() {
        assert_eq!(
            go("{s:upper}", json!({"s": "go"}), "s: { type: string }").unwrap(),
            "GO"
        );
        assert_eq!(percent_encode("#PTS 50/50"), "%23PTS%2050%2F50");
        // Literal JSON braces are not placeholders.
        assert_eq!(
            go(
                r#"{"status":"toggle","n":{s}}"#,
                json!({"s": "1"}),
                "s: { type: string }"
            )
            .unwrap(),
            r#"{"status":"toggle","n":1}"#
        );
        assert_eq!(
            go("{s:url}", json!({"s": "Cam 1 & 2"}), "s: { type: string }").unwrap(),
            "Cam%201%20%26%202"
        );
    }
}
