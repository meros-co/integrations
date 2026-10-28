//! Credentials computed for each request (SPEC.md §2, "Computed request
//! credentials", "AWS Signature Version 4" and "WS-Security UsernameToken"):
//! values a device wants worked out from the request, the time and a secret
//! rather than sent as they are.
//!
//! - `transport.sign`: a short list of named values, each one of a closed set
//!   of expressions (the Unix time, a random number, a hash or an HMAC of a
//!   template), sent as headers or query parameters (AWS Elemental Live's
//!   `X-Auth-Key`, Nimble Streamer's `salt` and `hash`).
//! - `auth: aws-sigv4`: AWS Signature Version 4 over the whole request.
//! - `auth: wsse`: a WS-Security UsernameToken with a password digest in the
//!   SOAP envelope's header (ONVIF's legacy authentication).
//!
//! None of these values is logged, offered to the telemetry rules or kept in
//! state.

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::Value;

use crate::catalog::{ParamSpec, Params};
use crate::module::HttpRequest;

use super::template::{no_escape, percent_encode, render, Conversions, Maps, Values};

// ── Randomness and the clock ─────────────────────────────────────────────

#[cfg(test)]
thread_local! {
    /// Tests and vectors draw "random" bytes from this counter instead, so
    /// what they send can be stated in advance.
    static FIXED_RANDOM: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}

/// Make this thread's random bytes predictable: each byte drawn is the next
/// value of a counter starting at `start`.
#[cfg(test)]
pub(crate) fn fix_random(start: Option<u8>) {
    FIXED_RANDOM.with(|f| f.set(start));
}

/// Fill `buf` from the operating system's generator.
pub(crate) fn random_bytes(buf: &mut [u8]) {
    #[cfg(test)]
    {
        if let Some(mut next) = FIXED_RANDOM.with(|f| f.get()) {
            for b in buf.iter_mut() {
                *b = next;
                next = next.wrapping_add(1);
            }
            FIXED_RANDOM.with(|f| f.set(Some(next)));
            return;
        }
    }
    use ring::rand::SecureRandom;
    if ring::rand::SystemRandom::new().fill(buf).is_err() {
        // No generator: a value that is at least different each time.
        let seed = std::collections::hash_map::RandomState::new();
        use std::hash::{BuildHasher, Hasher};
        for (i, b) in buf.iter_mut().enumerate() {
            let mut h = seed.build_hasher();
            h.write_usize(i);
            *b = h.finish() as u8;
        }
    }
}

/// The UTC date and time of `unix` seconds: (year, month, day, hour,
/// minute, second).
fn civil(unix: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        month,
        day,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
        (secs % 60) as u32,
    )
}

/// `20150830T123600Z`, the form AWS's `X-Amz-Date` takes.
pub(crate) fn amz_date(unix: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(unix);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// `2015-08-30T12:36:00Z`, XML Schema's dateTime in UTC.
pub(crate) fn iso_date(unix: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(unix);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

// ── Digests ──────────────────────────────────────────────────────────────

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn md5(data: &[u8]) -> Vec<u8> {
    use md5::Digest;
    md5::Md5::digest(data).to_vec()
}

fn sha1(data: &[u8]) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, data)
        .as_ref()
        .to_vec()
}

fn sha256(data: &[u8]) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .to_vec()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    ring::hmac::sign(&key, data).as_ref().to_vec()
}

// ── The request's parts ──────────────────────────────────────────────────

/// `scheme://authority`, the path and the query (without `?`) of a URL.
fn split(url: &str) -> (&str, &str, &str) {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let path_at = url[after_scheme..]
        .find('/')
        .map_or(url.len(), |i| after_scheme + i);
    let origin = &url[..path_at];
    let rest = &url[path_at..];
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    (origin, if path.is_empty() { "/" } else { path }, query)
}

// ── transport.sign ───────────────────────────────────────────────────────

/// How a digest is written.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Encoding {
    Hex,
    Base64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Digest {
    Md5,
    Sha1,
    Sha256,
}

#[derive(Debug)]
enum Expr {
    /// `unix_time: <offset s>`: whole seconds since 1970-01-01 UTC, plus the
    /// offset.
    UnixTime(i64),
    /// `random: { min, max }`: a whole number drawn evenly from the range.
    Random(u64, u64),
    /// `text: <template>`, with the first match of `remove` taken out.
    Text(String, Option<Regex>),
    /// `md5`, `sha1` or `sha256`: a template's digest.
    Hash(Digest, String, Encoding),
    /// `hmac_sha256: <template>` keyed by `key`, a template too.
    Hmac(String, String, Encoding),
}

/// `transport.sign`: named values worked out for each request, in order,
/// and the headers and query parameters they are sent in.
#[derive(Debug)]
pub(crate) struct Sign {
    when_set: Option<String>,
    values: Vec<(String, Expr)>,
    headers: Vec<(String, String)>,
    query: Vec<(String, String)>,
}

/// The names a sign template may use besides its own values.
const REQUEST_NAMES: [&str; 4] = [
    "request.method",
    "request.path",
    "request.query",
    "request.body",
];

impl Sign {
    pub(crate) fn parse(v: &Value) -> Result<Sign, String> {
        let bad = |what: &str| format!("transport.sign: {what}");
        let mut values = Vec::new();
        let map = match v.get("values") {
            None => serde_json::Map::new(),
            Some(Value::Object(m)) => m.clone(),
            Some(_) => return Err(bad("values maps names to expressions")),
        };
        for (name, e) in &map {
            let valid = !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !valid {
                return Err(bad(&format!("'{name}' is not a snake_case name")));
            }
            let o = e
                .as_object()
                .filter(|o| !o.is_empty())
                .ok_or_else(|| bad(&format!("values.{name} is an expression")))?;
            let encoding = match o.get("encoding").and_then(Value::as_str) {
                None | Some("hex") => Encoding::Hex,
                Some("base64") => Encoding::Base64,
                Some(other) => {
                    return Err(bad(&format!(
                        "values.{name}.encoding '{other}' is hex or base64"
                    )))
                }
            };
            let template = |key: &str| -> Result<String, String> {
                o.get(key)
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| bad(&format!("values.{name}.{key} is a template")))
            };
            let expr = if let Some(offset) = o.get("unix_time") {
                Expr::UnixTime(
                    offset
                        .as_i64()
                        .ok_or_else(|| bad(&format!("values.{name}.unix_time is seconds")))?,
                )
            } else if let Some(r) = o.get("random") {
                let min = r.get("min").and_then(Value::as_u64).unwrap_or(0);
                let max = r
                    .get("max")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| bad(&format!("values.{name}.random needs max")))?;
                if max < min {
                    return Err(bad(&format!("values.{name}.random: max is below min")));
                }
                Expr::Random(min, max)
            } else if o.contains_key("text") {
                let remove = match o.get("remove").and_then(Value::as_str) {
                    Some(re) => Some(
                        Regex::new(re).map_err(|e| bad(&format!("values.{name}.remove: {e}")))?,
                    ),
                    None => None,
                };
                Expr::Text(template("text")?, remove)
            } else if o.contains_key("md5") {
                Expr::Hash(Digest::Md5, template("md5")?, encoding)
            } else if o.contains_key("sha1") {
                Expr::Hash(Digest::Sha1, template("sha1")?, encoding)
            } else if o.contains_key("sha256") {
                Expr::Hash(Digest::Sha256, template("sha256")?, encoding)
            } else if o.contains_key("hmac_sha256") {
                Expr::Hmac(template("hmac_sha256")?, template("key")?, encoding)
            } else {
                return Err(bad(&format!(
                    "values.{name} is one of unix_time, random, text, md5, sha1, sha256 or hmac_sha256"
                )));
            };
            values.push((name.clone(), expr));
        }
        let pairs = |key: &str| -> Result<Vec<(String, String)>, String> {
            let mut out = Vec::new();
            match v.get(key) {
                None => {}
                Some(Value::Object(m)) => {
                    for (k, t) in m {
                        let t = t
                            .as_str()
                            .ok_or_else(|| bad(&format!("{key}.{k} is a template")))?;
                        out.push((k.clone(), t.to_string()));
                    }
                }
                Some(_) => return Err(bad(&format!("{key} maps names to templates"))),
            }
            Ok(out)
        };
        let headers = pairs("headers")?;
        for (name, _) in &headers {
            let valid =
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
            if !valid
                || ["authorization", "content-type", "host"]
                    .contains(&name.to_ascii_lowercase().as_str())
            {
                return Err(bad(&format!("headers: '{name}' cannot be set here")));
            }
        }
        let query = pairs("query")?;
        if headers.is_empty() && query.is_empty() {
            return Err(bad("sends nothing: give headers or query"));
        }
        Ok(Sign {
            when_set: v
                .get("when_set")
                .and_then(Value::as_str)
                .map(str::to_string),
            values,
            headers,
            query,
        })
    }

    /// Whether the values are worked out now: always, or while the
    /// `when_set` setting is set.
    fn applies(&self, settings: &Params) -> bool {
        match &self.when_set {
            None => true,
            Some(name) => settings.get(name).is_some_and(|v| {
                !v.is_null() && v.as_str() != Some("") && v.as_bool() != Some(false)
            }),
        }
    }

    /// Work the values out for `request` at `unix_ms` and add its headers
    /// and query parameters.
    pub(crate) fn apply(
        &self,
        request: &mut HttpRequest,
        settings: &Params,
        setting_specs: &BTreeMap<String, ParamSpec>,
        unix_ms: u64,
    ) -> Result<(), String> {
        if !self.applies(settings) {
            return Ok(());
        }
        let string: ParamSpec =
            serde_json::from_value(serde_json::json!({"type": "string"})).unwrap();
        let (_, path, query) = split(&request.url);
        let mut params = Params::new();
        params.insert("request.method".into(), request.method.into());
        params.insert("request.path".into(), path.into());
        params.insert("request.query".into(), query.into());
        params.insert(
            "request.body".into(),
            request
                .body
                .as_deref()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default()
                .into(),
        );
        let mut specs: BTreeMap<String, ParamSpec> = REQUEST_NAMES
            .iter()
            .map(|n| (n.to_string(), string.clone()))
            .collect();
        let (conversions, maps) = (Conversions::default(), Maps::default());
        let rendered = |t: &str, params: &Params, specs: &BTreeMap<String, ParamSpec>| {
            render(
                t,
                &Values {
                    params,
                    param_specs: specs,
                    settings,
                    setting_specs,
                    conversions: &conversions,
                    maps: &maps,
                    session: None,
                },
                no_escape,
            )
        };
        for (name, expr) in &self.values {
            let encode = |bytes: Vec<u8>, e: Encoding| match e {
                Encoding::Hex => hex(&bytes),
                Encoding::Base64 => base64(&bytes),
            };
            let value = match expr {
                Expr::UnixTime(offset) => ((unix_ms / 1000) as i64 + offset).to_string(),
                Expr::Random(min, max) => {
                    let mut b = [0u8; 8];
                    random_bytes(&mut b);
                    let span = max - min;
                    let n = u64::from_le_bytes(b);
                    (min + if span == u64::MAX { n } else { n % (span + 1) }).to_string()
                }
                Expr::Text(t, remove) => {
                    let text = rendered(t, &params, &specs)?;
                    match remove {
                        Some(re) => re.replace(&text, "").into_owned(),
                        None => text,
                    }
                }
                Expr::Hash(d, t, e) => {
                    let data = rendered(t, &params, &specs)?.into_bytes();
                    encode(
                        match d {
                            Digest::Md5 => md5(&data),
                            Digest::Sha1 => sha1(&data),
                            Digest::Sha256 => sha256(&data),
                        },
                        *e,
                    )
                }
                Expr::Hmac(t, key, e) => {
                    let data = rendered(t, &params, &specs)?;
                    let key = rendered(key, &params, &specs)?;
                    encode(hmac_sha256(key.as_bytes(), data.as_bytes()), *e)
                }
            };
            params.insert(name.clone(), value.into());
            specs.insert(name.clone(), string.clone());
        }
        for (name, t) in &self.headers {
            let value = rendered(t, &params, &specs)?;
            if value.contains(['\r', '\n']) {
                return Err(format!("header {name} contains a line break"));
            }
            request.headers.push((name.clone(), value));
        }
        if !self.query.is_empty() {
            let mut pairs = Vec::new();
            for (k, t) in &self.query {
                let v = rendered(t, &params, &specs)?;
                pairs.push(format!("{}={}", percent_encode(k), percent_encode(&v)));
            }
            let sep = if request.url.contains('?') { '&' } else { '?' };
            request.url.push(sep);
            request.url.push_str(&pairs.join("&"));
        }
        Ok(())
    }
}

// ── AWS Signature Version 4 ──────────────────────────────────────────────

/// `transport.aws`: the service and region a request is signed for.
#[derive(Debug)]
pub(crate) struct Aws {
    service: String,
    /// The region, or the setting naming it.
    region: Result<String, String>,
}

impl Aws {
    pub(crate) fn parse(v: Option<&Value>) -> Result<Aws, String> {
        let v = v.ok_or("auth aws-sigv4 needs transport.aws")?;
        let service = v
            .get("service")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
            .ok_or("transport.aws.service is the service's signing name, such as medialive")?
            .to_string();
        let region = match v.get("region") {
            Some(Value::String(r)) => Ok(r.clone()),
            Some(Value::Object(o)) => Err(o
                .get("setting")
                .and_then(Value::as_str)
                .ok_or("transport.aws.region is a region or {setting: name}")?
                .to_string()),
            _ => return Err("transport.aws.region is a region or {setting: name}".into()),
        };
        Ok(Aws { service, region })
    }

    fn region(&self, settings: &Params) -> String {
        match &self.region {
            Ok(r) => r.clone(),
            Err(name) => settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        }
    }

    /// Sign `request` at `unix_ms` with the `access_key_id`,
    /// `secret_access_key` and optional `session_token` settings: adds
    /// `Host`, `X-Amz-Date`, `X-Amz-Security-Token` when there is a session
    /// token, and `Authorization`.
    pub(crate) fn sign(&self, request: &mut HttpRequest, settings: &Params, unix_ms: u64) {
        let setting = |name: &str| {
            settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let (key_id, secret, token) = (
            setting("access_key_id"),
            setting("secret_access_key"),
            setting("session_token"),
        );
        let region = self.region(settings);
        let date = amz_date((unix_ms / 1000) as i64);
        let day = &date[..8];
        let (origin, path, query) = split(&request.url);
        let authority = origin.split_once("://").map_or(origin, |(_, a)| a);
        let host = authority
            .strip_suffix(":443")
            .filter(|_| origin.starts_with("https://"))
            .or_else(|| {
                authority
                    .strip_suffix(":80")
                    .filter(|_| origin.starts_with("http://"))
            })
            .unwrap_or(authority)
            .to_string();
        let path = path.to_string();
        let query = query.to_string();
        request.headers.retain(|(n, _)| {
            !n.eq_ignore_ascii_case("host") && !n.eq_ignore_ascii_case("authorization")
        });
        request.headers.push(("Host".into(), host));
        request.headers.push(("X-Amz-Date".into(), date.clone()));
        if !token.is_empty() {
            request.headers.push(("X-Amz-Security-Token".into(), token));
        }
        let payload = match (&request.body, &request.body_stream) {
            (_, Some(_)) => {
                request
                    .headers
                    .push(("X-Amz-Content-Sha256".into(), "UNSIGNED-PAYLOAD".into()));
                "UNSIGNED-PAYLOAD".to_string()
            }
            (Some(body), None) => hex(&sha256(body)),
            (None, None) => hex(&sha256(b"")),
        };
        // Host, the content type and every x-amz- header are signed.
        let mut signed: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|(n, v)| (n.to_ascii_lowercase(), v.clone()))
            .filter(|(n, _)| n == "host" || n == "content-type" || n.starts_with("x-amz-"))
            .map(|(n, v)| (n, v.split_whitespace().collect::<Vec<_>>().join(" ")))
            .collect();
        signed.sort();
        let canonical_headers: String = signed.iter().map(|(n, v)| format!("{n}:{v}\n")).collect();
        let signed_names = signed
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            request.method,
            canonical_uri(&path),
            canonical_query(&query),
            canonical_headers,
            signed_names,
            payload
        );
        let scope = format!("{day}/{region}/{}/aws4_request", self.service);
        let to_sign = format!(
            "AWS4-HMAC-SHA256\n{date}\n{scope}\n{}",
            hex(&sha256(canonical.as_bytes()))
        );
        let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), day.as_bytes());
        let k_region = hmac_sha256(&k_date, region.as_bytes());
        let k_service = hmac_sha256(&k_region, self.service.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let signature = hex(&hmac_sha256(&k_signing, to_sign.as_bytes()));
        request.headers.push((
            "Authorization".into(),
            format!(
                "AWS4-HMAC-SHA256 Credential={key_id}/{scope}, SignedHeaders={signed_names}, Signature={signature}"
            ),
        ));
    }
}

/// AWS's URI encoding: unreserved characters kept, every other byte as
/// `%XX` in upper case; `/` kept when `slash` is.
fn aws_encode(s: &str, slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (slash && b == b'/')
        {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The canonical URI of a path as sent: each segment, already
/// percent-encoded, is encoded once more, as every service but S3 wants.
fn canonical_uri(path: &str) -> String {
    let path = if path.is_empty() { "/" } else { path };
    aws_encode(path, true)
}

/// The canonical query: each name and value decoded, encoded AWS's way,
/// and the pairs sorted.
fn canonical_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (
                aws_encode(&super::template::percent_decode(k), false),
                aws_encode(&super::template::percent_decode(v), false),
            )
        })
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

// ── WS-Security UsernameToken ────────────────────────────────────────────

const WSSE_NS: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const WSU_NS: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
const DIGEST_TYPE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest";
const NONCE_TYPE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The `wsse:Security` header for `username` and `password` at `unix_ms`:
/// a fresh 16-byte nonce, the time, and the password digest
/// Base64(SHA-1(nonce + created + password)) (OASIS Username Token Profile
/// 1.0, §3.1), with `soap` the envelope's namespace prefix.
fn security_header(soap: &str, username: &str, password: &str, unix_ms: u64) -> String {
    let mut nonce = [0u8; 16];
    random_bytes(&mut nonce);
    let created = iso_date((unix_ms / 1000) as i64);
    let mut data = nonce.to_vec();
    data.extend_from_slice(created.as_bytes());
    data.extend_from_slice(password.as_bytes());
    let digest = base64(&sha1(&data));
    format!(
        "<wsse:Security {soap}:mustUnderstand=\"1\" xmlns:wsse=\"{WSSE_NS}\" xmlns:wsu=\"{WSU_NS}\"><wsse:UsernameToken><wsse:Username>{}</wsse:Username><wsse:Password Type=\"{DIGEST_TYPE}\">{digest}</wsse:Password><wsse:Nonce EncodingType=\"{NONCE_TYPE}\">{}</wsse:Nonce><wsu:Created>{created}</wsu:Created></wsse:UsernameToken></wsse:Security>",
        xml_escape(username),
        base64(&nonce),
    )
}

/// Put a UsernameToken in the SOAP envelope of `request`'s body, from the
/// `username` and `password` settings: inside the envelope's `Header`, or
/// in a new one before its `Body`. A body that is not a SOAP envelope, or
/// an empty user name, is left as it is.
pub(crate) fn wsse(request: &mut HttpRequest, settings: &Params, unix_ms: u64) {
    let setting = |name: &str| {
        settings
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let username = setting("username");
    if username.is_empty() {
        return;
    }
    let Some(body) = request.body.as_ref() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(body) else {
        return;
    };
    static ENVELOPE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let envelope =
        ENVELOPE.get_or_init(|| Regex::new(r"<([A-Za-z_][\w.-]*):Envelope[\s>]").unwrap());
    let Some(prefix) = envelope.captures(text).map(|c| c[1].to_string()) else {
        return;
    };
    let security = security_header(&prefix, &username, &setting("password"), unix_ms);
    let header_open = Regex::new(&format!(r"<{prefix}:Header(\s[^>]*)?>")).unwrap();
    let out = if let Some(m) = header_open.find(text) {
        format!("{}{security}{}", &text[..m.end()], &text[m.end()..])
    } else if let Some(at) = text.find(&format!("<{prefix}:Body")) {
        format!(
            "{}<{prefix}:Header>{security}</{prefix}:Header>{}",
            &text[..at],
            &text[at..]
        )
    } else {
        return;
    };
    request.body = Some(out.into_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(method: &'static str, url: &str, body: Option<&str>) -> HttpRequest {
        HttpRequest {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: body.map(|b| b.as_bytes().to_vec()),
            body_stream: None,
            timeout: Some(1000),
            accept_invalid_certs: false,
            digest: None,
        }
    }

    fn settings(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn header<'a>(r: &'a HttpRequest, name: &str) -> &'a str {
        r.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .unwrap()
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(amz_date(0), "19700101T000000Z");
        assert_eq!(amz_date(1_440_938_160), "20150830T123600Z");
        assert_eq!(iso_date(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_date(1_798_761_599), "2026-12-31T23:59:59Z");
    }

    /// AWS's Signature Version 4 test suite, get-vanilla and
    /// post-x-www-form-urlencoded.
    #[test]
    fn signature_version_4_matches_the_aws_test_suite() {
        let aws = Aws::parse(Some(&json!({"service": "service", "region": "us-east-1"}))).unwrap();
        let creds = settings(json!({
            "access_key_id": "AKIDEXAMPLE",
            "secret_access_key": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        }));
        let at = 1_440_938_160_000;
        let mut r = request("GET", "https://example.amazonaws.com:443/", None);
        aws.sign(&mut r, &creds, at);
        assert_eq!(header(&r, "host"), "example.amazonaws.com");
        assert_eq!(header(&r, "x-amz-date"), "20150830T123600Z");
        assert_eq!(
            header(&r, "authorization"),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
        let mut r = request(
            "GET",
            "https://example.amazonaws.com/?Param2=value2&Param1=value1",
            None,
        );
        aws.sign(&mut r, &creds, at);
        assert_eq!(
            header(&r, "authorization"),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500"
        );
    }

    #[test]
    fn a_session_token_is_sent_and_signed() {
        let aws = Aws::parse(Some(
            &json!({"service": "medialive", "region": {"setting": "region"}}),
        ))
        .unwrap();
        let creds = settings(json!({
            "access_key_id": "AKID", "secret_access_key": "secret",
            "session_token": "tok en", "region": "eu-west-1",
        }));
        let mut r = request(
            "POST",
            "https://medialive.eu-west-1.amazonaws.com:443/prod/channels/1/start",
            None,
        );
        aws.sign(&mut r, &creds, 0);
        assert_eq!(header(&r, "x-amz-security-token"), "tok en");
        let auth = header(&r, "authorization");
        assert!(
            auth.contains("/19700101/eu-west-1/medialive/aws4_request"),
            "{auth}"
        );
        assert!(
            auth.contains("SignedHeaders=host;x-amz-date;x-amz-security-token,"),
            "{auth}"
        );
    }

    #[test]
    fn paths_and_queries_are_canonical() {
        assert_eq!(
            canonical_uri("/prod/tags/arn%3Aaws%3Amedialive"),
            "/prod/tags/arn%253Aaws%253Amedialive"
        );
        assert_eq!(canonical_query("b=2&a=%20x&a=1"), "a=%20x&a=1&b=2");
        assert_eq!(canonical_query(""), "");
    }

    #[test]
    fn signed_values_are_worked_out_in_order() {
        let sign = Sign::parse(&json!({
            "when_set": "api_key",
            "values": {
                "expires": {"unix_time": 30},
                "url": {"text": "{request.path}", "remove": "^/api(?:/v[0-9][^/]*)?"},
                "inner": {"md5": "{url}{settings.user}{settings.api_key}{expires}"},
                "key": {"md5": "{settings.api_key}{inner}"},
            },
            "headers": {"X-Auth-User": "{settings.user}", "X-Auth-Expires": "{expires}", "X-Auth-Key": "{key}"},
        }))
        .unwrap();
        let specs: BTreeMap<String, ParamSpec> = ["user", "api_key"]
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    serde_json::from_value(json!({"type": "string"})).unwrap(),
                )
            })
            .collect();
        let s = settings(json!({"user": "admin", "api_key": "k"}));
        let mut r = request(
            "GET",
            "http://10.0.0.1:80/api/live_events/3/status?x=1",
            None,
        );
        sign.apply(&mut r, &s, &specs, 1_000_000).unwrap();
        let inner = hex(&md5(b"/live_events/3/statusadmink1030"));
        let key = hex(&md5(format!("k{inner}").as_bytes()));
        assert_eq!(header(&r, "X-Auth-Expires"), "1030");
        assert_eq!(header(&r, "X-Auth-Key"), key);
        // Without the key, nothing is added.
        let mut r = request("GET", "http://10.0.0.1:80/api/x", None);
        sign.apply(
            &mut r,
            &settings(json!({"user": "admin", "api_key": ""})),
            &specs,
            0,
        )
        .unwrap();
        assert!(r.headers.is_empty());
    }

    #[test]
    fn a_random_salt_and_a_base64_digest_go_in_the_query() {
        let sign = Sign::parse(&json!({
            "values": {
                "salt": {"random": {"max": 1000000}},
                "hash": {"md5": "{salt}/{settings.token}", "encoding": "base64"},
            },
            "query": {"salt": "{salt}", "hash": "{hash}"},
        }))
        .unwrap();
        let specs: BTreeMap<String, ParamSpec> = [(
            "token".to_string(),
            serde_json::from_value(json!({"type": "string"})).unwrap(),
        )]
        .into();
        fix_random(Some(0));
        let mut r = request("GET", "http://h:8082/manage/server_status", None);
        sign.apply(&mut r, &settings(json!({"token": "t"})), &specs, 0)
            .unwrap();
        fix_random(None);
        let salt = u64::from_le_bytes([0, 1, 2, 3, 4, 5, 6, 7]) % 1_000_001;
        let hash = percent_encode(&base64(&md5(format!("{salt}/t").as_bytes())));
        assert_eq!(
            r.url,
            format!("http://h:8082/manage/server_status?salt={salt}&hash={hash}")
        );
    }

    #[test]
    fn a_username_token_goes_into_the_soap_header() {
        let body = r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Body><x/></s:Body></s:Envelope>"#;
        let mut r = request("POST", "http://cam/onvif/device_service", Some(body));
        fix_random(Some(0));
        wsse(
            &mut r,
            &settings(json!({"username": "a<b", "password": "pw"})),
            1_000,
        );
        fix_random(None);
        let text = String::from_utf8(r.body.unwrap()).unwrap();
        let nonce: Vec<u8> = (0..16).collect();
        let mut data = nonce.clone();
        data.extend_from_slice(b"1970-01-01T00:00:01Z");
        data.extend_from_slice(b"pw");
        assert!(
            text.contains("<s:Header><wsse:Security s:mustUnderstand=\"1\""),
            "{text}"
        );
        assert!(text.contains("<wsse:Username>a&lt;b</wsse:Username>"));
        assert!(text.contains(&format!(
            "#PasswordDigest\">{}</wsse:Password>",
            base64(&sha1(&data))
        )));
        assert!(text.contains(&format!("#Base64Binary\">{}</wsse:Nonce>", base64(&nonce))));
        assert!(text.contains("<wsu:Created>1970-01-01T00:00:01Z</wsu:Created></wsse:UsernameToken></wsse:Security></s:Header><s:Body>"));
        // An envelope with a header keeps it, the token first inside.
        let body = r#"<e:Envelope xmlns:e="x"><e:Header><a/></e:Header><e:Body/></e:Envelope>"#;
        let mut r = request("POST", "http://cam/x", Some(body));
        wsse(
            &mut r,
            &settings(json!({"username": "u", "password": "p"})),
            0,
        );
        let text = String::from_utf8(r.body.unwrap()).unwrap();
        assert!(
            text.starts_with("<e:Envelope xmlns:e=\"x\"><e:Header><wsse:Security e:mustUnderstand")
        );
        assert!(text.ends_with("</wsse:Security><a/></e:Header><e:Body/></e:Envelope>"));
    }
}
