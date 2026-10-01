//! HTTP Digest access authentication (RFC 7616, and RFC 2069's form when the
//! challenge offers no `qop`): challenge parsing and the Authorization header.
//!
//! Supported: MD5, MD5-sess, SHA-256 and SHA-256-sess with `qop=auth`. A
//! challenge offering only `auth-int`, or another algorithm, is not answered,
//! so the 401 stands and the engine treats it as a refusal.

use std::sync::atomic::{AtomicU64, Ordering};

use md5::Md5;
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Algorithm {
    Md5,
    Md5Sess,
    Sha256,
    Sha256Sess,
}

impl Algorithm {
    fn parse(name: &str) -> Option<Algorithm> {
        match name.to_ascii_uppercase().as_str() {
            "MD5" => Some(Algorithm::Md5),
            "MD5-SESS" => Some(Algorithm::Md5Sess),
            "SHA-256" => Some(Algorithm::Sha256),
            "SHA-256-SESS" => Some(Algorithm::Sha256Sess),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Algorithm::Md5 => "MD5",
            Algorithm::Md5Sess => "MD5-sess",
            Algorithm::Sha256 => "SHA-256",
            Algorithm::Sha256Sess => "SHA-256-sess",
        }
    }

    fn hash(self, data: &str) -> String {
        match self {
            Algorithm::Md5 | Algorithm::Md5Sess => hex(&Md5::digest(data.as_bytes())),
            Algorithm::Sha256 | Algorithm::Sha256Sess => hex(&Sha256::digest(data.as_bytes())),
        }
    }

    fn session(self) -> bool {
        matches!(self, Algorithm::Md5Sess | Algorithm::Sha256Sess)
    }

    /// Preference when a server offers several challenges.
    fn strength(self) -> u8 {
        match self {
            Algorithm::Sha256 | Algorithm::Sha256Sess => 2,
            Algorithm::Md5 | Algorithm::Md5Sess => 1,
        }
    }
}

/// A Digest challenge the core can answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Challenge {
    pub(crate) realm: String,
    pub(crate) nonce: String,
    pub(crate) opaque: Option<String>,
    pub(crate) algorithm: Algorithm,
    /// `qop=auth` offered; otherwise the RFC 2069 form.
    pub(crate) qop_auth: bool,
}

/// The strongest answerable Digest challenge among `WWW-Authenticate` values.
pub(crate) fn choose<'a>(headers: impl IntoIterator<Item = &'a str>) -> Option<Challenge> {
    headers
        .into_iter()
        .filter_map(parse)
        .max_by_key(|c| c.algorithm.strength())
}

/// One `WWW-Authenticate` value, if it is a Digest challenge the core can
/// answer.
pub(crate) fn parse(header: &str) -> Option<Challenge> {
    let header = header.trim_start();
    let (scheme, rest) = header.split_once(char::is_whitespace)?;
    if !scheme.eq_ignore_ascii_case("digest") {
        return None;
    }
    let params = params(rest);
    let get = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    };
    let algorithm = match get("algorithm") {
        Some(a) => Algorithm::parse(&a)?,
        None => Algorithm::Md5,
    };
    let qop_auth = match get("qop") {
        None => false,
        Some(q) => {
            if !q.split(',').any(|o| o.trim().eq_ignore_ascii_case("auth")) {
                return None;
            }
            true
        }
    };
    Some(Challenge {
        realm: get("realm").unwrap_or_default(),
        nonce: get("nonce")?,
        opaque: get("opaque"),
        algorithm,
        qop_auth,
    })
}

/// `name=value` and `name="quoted value"` pairs, comma separated.
fn params(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| *c == ',' || c.is_whitespace()) {
            chars.next();
        }
        let mut name = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c == ',' {
                break;
            }
            name.push(c);
            chars.next();
        }
        if name.is_empty() {
            return out;
        }
        if chars.next() != Some('=') {
            continue;
        }
        let mut value = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(escaped) = chars.next() {
                            value.push(escaped);
                        }
                    }
                    '"' => break,
                    c => value.push(c),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == ',' {
                    break;
                }
                value.push(c);
                chars.next();
            }
        }
        out.push((name.trim().to_string(), value.trim().to_string()));
    }
}

/// The Authorization header value answering `challenge` for one request.
/// `uri` is the request target (path and query); `nc` counts requests made
/// with this nonce, from 1.
pub(crate) fn authorization(
    challenge: &Challenge,
    username: &str,
    password: &str,
    method: &str,
    uri: &str,
    nc: u32,
    cnonce: &str,
) -> String {
    let alg = challenge.algorithm;
    let mut ha1 = alg.hash(&format!("{username}:{}:{password}", challenge.realm));
    if alg.session() {
        ha1 = alg.hash(&format!("{ha1}:{}:{cnonce}", challenge.nonce));
    }
    let ha2 = alg.hash(&format!("{method}:{uri}"));
    let nc = format!("{nc:08x}");
    let response = if challenge.qop_auth {
        alg.hash(&format!(
            "{ha1}:{}:{nc}:{cnonce}:auth:{ha2}",
            challenge.nonce
        ))
    } else {
        alg.hash(&format!("{ha1}:{}:{ha2}", challenge.nonce))
    };
    let mut header = format!(
        "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", algorithm={}, response=\"{response}\"",
        quote(username),
        quote(&challenge.realm),
        quote(&challenge.nonce),
        quote(uri),
        alg.name(),
    );
    if let Some(opaque) = &challenge.opaque {
        header.push_str(&format!(", opaque=\"{}\"", quote(opaque)));
    }
    if challenge.qop_auth {
        header.push_str(&format!(", qop=auth, nc={nc}, cnonce=\"{cnonce}\""));
    }
    header
}

/// A fresh client nonce: unpredictable enough to keep requests distinct.
pub(crate) fn cnonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    hex(&Sha256::digest(format!("{now}:{n}:{}", std::process::id()).as_bytes())[..16])
}

fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_of(header: &str) -> &str {
        let at = header.find("response=\"").unwrap() + "response=\"".len();
        &header[at..at + header[at..].find('"').unwrap()]
    }

    /// RFC 2617 §3.5: MD5 with qop=auth.
    #[test]
    fn rfc_2617_example() {
        let c = parse(
            "Digest realm=\"testrealm@host.com\", qop=\"auth,auth-int\", \
             nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", \
             opaque=\"5ccc069c403ebaf9f0171e9517f40e41\"",
        )
        .unwrap();
        assert_eq!(c.algorithm, Algorithm::Md5);
        assert!(c.qop_auth);
        let h = authorization(
            &c,
            "Mufasa",
            "Circle Of Life",
            "GET",
            "/dir/index.html",
            1,
            "0a4f113b",
        );
        assert_eq!(response_of(&h), "6629fae49393a05397450978507c4ef1");
        assert!(h.contains("nc=00000001"));
        assert!(h.contains("opaque=\"5ccc069c403ebaf9f0171e9517f40e41\""));
    }

    /// RFC 7616 §3.9.1: the same request answered with SHA-256 and with MD5.
    #[test]
    fn rfc_7616_examples() {
        let challenge = |alg: &str| {
            parse(&format!(
                "Digest realm=\"http-auth@example.org\", qop=\"auth, auth-int\", \
                 algorithm={alg}, nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
                 opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\""
            ))
            .unwrap()
        };
        let cnonce = "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ";
        let answer = |c: &Challenge| {
            authorization(
                c,
                "Mufasa",
                "Circle of Life",
                "GET",
                "/dir/index.html",
                1,
                cnonce,
            )
        };
        assert_eq!(
            response_of(&answer(&challenge("SHA-256"))),
            "753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1"
        );
        assert_eq!(
            response_of(&answer(&challenge("MD5"))),
            "8ca523f5e9506fed4657c9700eebdbec"
        );
    }

    #[test]
    fn choice_and_refusals() {
        // Strongest answerable challenge wins; Basic is ignored.
        let c = choose([
            "Basic realm=\"cam\"",
            "Digest realm=\"cam\", nonce=\"n1\", qop=\"auth\"",
            "Digest realm=\"cam\", nonce=\"n2\", qop=\"auth\", algorithm=SHA-256",
        ])
        .unwrap();
        assert_eq!((c.nonce.as_str(), c.algorithm), ("n2", Algorithm::Sha256));
        // auth-int only, or an unknown algorithm, cannot be answered.
        assert_eq!(
            parse("Digest realm=\"r\", nonce=\"n\", qop=\"auth-int\""),
            None
        );
        assert_eq!(
            parse("Digest realm=\"r\", nonce=\"n\", algorithm=SHA-512-256"),
            None
        );
        // No qop: the RFC 2069 form, without nc or cnonce.
        let c = parse("digest realm=\"r\", nonce=\"n\"").unwrap();
        let h = authorization(&c, "u", "p", "GET", "/", 1, "x");
        assert!(!c.qop_auth && !h.contains("nc="));
    }

    #[test]
    fn quoted_values_and_escapes() {
        let p = params(r#"realm="a \"b\", c", nonce=abc , opaque="""#);
        assert_eq!(
            p,
            vec![
                ("realm".into(), "a \"b\", c".into()),
                ("nonce".into(), "abc".into()),
                ("opaque".into(), "".into()),
            ]
        );
    }
}
