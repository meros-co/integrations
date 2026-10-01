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
    RE.get_or_init(|| {
        Regex::new(r#"\{([A-Za-z0-9_][A-Za-z0-9_.]*)((?::[^{}:"\s]+)*)\}"#).unwrap()
    })
}

/// Where values come from: a command's parameters and the device's settings,
/// both already validated with defaults applied.
pub(crate) struct Values<'a> {
    pub(crate) params: &'a Params,
    pub(crate) param_specs: &'a BTreeMap<String, ParamSpec>,
    pub(crate) settings: &'a Params,
    pub(crate) setting_specs: &'a BTreeMap<String, ParamSpec>,
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

fn render_one(name: &str, directives: &[&str], values: &Values) -> Result<String, String> {
    let (value, kind) = values.lookup(name)?;

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
        let v = Values {
            params: &p,
            param_specs: &s,
            settings: &empty,
            setting_specs: &empty_specs,
        };
        render(template, &v, no_escape)
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
            go(r#"{"status":"toggle","n":{s}}"#, json!({"s": "1"}), "s: { type: string }")
                .unwrap(),
            r#"{"status":"toggle","n":1}"#
        );
        assert_eq!(
            go("{s:url}", json!({"s": "Cam 1 & 2"}), "s: { type: string }").unwrap(),
            "Cam%201%20%26%202"
        );
    }
}
