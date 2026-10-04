//! OAuth 2 for the HTTP transport (`auth: oauth2`, SPEC.md §2): keeping the
//! consumer's access token fresh with its refresh token (RFC 6749 §6).
//!
//! The consumer runs the sign-in and owns the tokens; they arrive as the
//! device's settings and are held in memory only. This is the refresh state
//! machine alone: when a refresh is due, the request that makes it, and what
//! its answer means. The engine sends the request and applies the outcome,
//! so every request of the device waits behind the one refresh in flight.
//!
//! `oauth.validate` adds the token check some services require of an
//! application on a schedule (Twitch: on start and hourly), independent of
//! the device's own traffic.

use base64::Engine as _;
use serde_json::{Map, Value};

use super::template::percent_encode;
use crate::catalog::Params;
use crate::module::{HttpRequest, HttpResponse, Millis, RequestId};

/// `oauth.refresh_ahead_s` when the spec gives none: refresh this long before
/// the access token expires.
const REFRESH_AHEAD_S: u64 = 300;
/// Backoff between refreshes that failed for a reason other than a refusal
/// (the token endpoint unreachable or failing).
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 60_000;
/// The longest token-endpoint error description repeated in a log line.
const DESCRIPTION_MAX: usize = 200;

/// How the client authenticates to the token endpoint (RFC 6749 §2.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientAuth {
    /// `client_id` and, when set, `client_secret` in the form body.
    Body,
    /// HTTP Basic with the form-encoded `client_id` and `client_secret`.
    Basic,
}

/// `oauth.validate.every_s` when the spec gives none: hourly.
const VALIDATE_EVERY_S: u64 = 3600;

/// `oauth.validate`: an endpoint the service requires the access token to be
/// checked at, once the device opens or the token is refreshed, then every
/// `every`.
#[derive(Debug, Clone)]
pub(crate) struct Validate {
    url: String,
    pub(crate) every: Millis,
    /// The `Authorization` scheme the token is sent with (Twitch documents
    /// `OAuth`).
    scheme: String,
}

/// What a finished token validation means.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Validated {
    /// The token is valid.
    Valid,
    /// HTTP 401: the token is not valid.
    Invalid,
    /// The check did not happen (network, another status): tried again
    /// after this long.
    Failed(String, Millis),
}

/// What a finished refresh means.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Refreshed {
    /// New settings: `access_token`, `expires_at` (Unix seconds, or null
    /// when the service gave no lifetime) and `refresh_token` when rotated.
    Tokens(Params),
    /// The token endpoint refused the refresh token or the client: terminal.
    Refused(String),
    /// The refresh did not happen (network, a server error): retried later.
    Failed(String),
}

#[derive(Debug, Clone)]
pub(crate) struct OAuth {
    /// The spec's token endpoint; the `token_url` setting overrides it.
    token_url: String,
    refresh_ahead: Millis,
    client_auth: ClientAuth,
    /// The refresh in flight, if any.
    pub(crate) refreshing: Option<RequestId>,
    /// The lifetime of the last token issued: a token living shorter than
    /// twice `refresh_ahead` is refreshed at half its life instead, so a
    /// short-lived token is not refreshed again at once.
    lifetime: Option<Millis>,
    /// After a failed refresh, none is started before this (session time).
    pub(crate) retry_at: Millis,
    pub(crate) backoff: Millis,
    /// Why the last refresh failed, for the commands that cannot wait for
    /// the next.
    pub(crate) last_error: Option<String>,
    /// `oauth.validate`, if the service requires it.
    pub(crate) validate: Option<Validate>,
    /// The validation in flight, if any.
    pub(crate) validating: Option<RequestId>,
    /// A validation is due once the refresh in flight (or due) is done.
    pub(crate) validate_waits: bool,
    /// The token was refreshed because validation refused it: a second
    /// refusal is final. Cleared by a valid answer.
    pub(crate) validate_refreshed: bool,
    validate_backoff: Millis,
}

fn setting<'a>(settings: &'a Params, name: &str) -> &'a str {
    settings.get(name).and_then(Value::as_str).unwrap_or("")
}

/// The expiry in Unix seconds: an integer setting, absent or null when
/// unknown.
fn expires_at(settings: &Params) -> Option<u64> {
    settings.get("expires_at").and_then(Value::as_u64)
}

/// A token endpoint the core sends a refresh token to: HTTPS, or plain HTTP
/// only to this machine (a product's local token proxy).
pub(crate) fn check_token_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") && url.len() > "https://".len() {
        return Ok(());
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let authority = rest.split('/').next().unwrap_or("");
        let host = if authority.starts_with('[') {
            authority.split(']').next().map(|h| format!("{h}]"))
        } else {
            authority.split(':').next().map(str::to_string)
        };
        let host = host.unwrap_or_default();
        let loopback = host == "localhost"
            || host == "[::1]"
            || host
                .parse::<std::net::Ipv4Addr>()
                .is_ok_and(|ip| ip.is_loopback());
        if loopback {
            return Ok(());
        }
    }
    Err(format!(
        "token URL '{url}' must be https, or http to this machine (localhost, 127.0.0.1 or [::1])"
    ))
}

impl OAuth {
    /// From the transport's `oauth` block.
    pub(crate) fn new(oauth: Option<&Value>) -> Result<OAuth, String> {
        let oauth = oauth.ok_or("auth: oauth2 needs an oauth block with token_url")?;
        let token_url = oauth
            .get("token_url")
            .and_then(Value::as_str)
            .ok_or("oauth needs a token_url")?
            .to_string();
        check_token_url(&token_url)?;
        let ahead = match oauth.get("refresh_ahead_s") {
            None => REFRESH_AHEAD_S,
            Some(v) => v
                .as_u64()
                .ok_or("oauth.refresh_ahead_s is a whole number of seconds")?,
        };
        let client_auth = match oauth.get("client_auth").and_then(Value::as_str) {
            None | Some("body") => ClientAuth::Body,
            Some("basic") => ClientAuth::Basic,
            Some(other) => return Err(format!("oauth.client_auth '{other}' is not body or basic")),
        };
        let validate = match oauth.get("validate") {
            None => None,
            Some(v) => Some(Validate::new(v)?),
        };
        Ok(OAuth {
            token_url,
            refresh_ahead: ahead * 1000,
            client_auth,
            refreshing: None,
            lifetime: None,
            retry_at: 0,
            backoff: RETRY_MIN,
            last_error: None,
            validate,
            validating: None,
            validate_waits: false,
            validate_refreshed: false,
            validate_backoff: RETRY_MIN,
        })
    }

    /// The validation request: a GET carrying the access token, and nothing
    /// else of the device's (no transport headers: it is another host).
    /// None without `oauth.validate` or without an access token.
    pub(crate) fn validate_request(
        &self,
        settings: &Params,
        timeout: Millis,
    ) -> Option<HttpRequest> {
        let validate = self.validate.as_ref()?;
        let token = setting(settings, "access_token");
        if token.is_empty() {
            return None;
        }
        Some(HttpRequest {
            method: "GET",
            url: validate.url.clone(),
            headers: vec![
                (
                    "Authorization".to_string(),
                    format!("{} {token}", validate.scheme),
                ),
                ("Accept".to_string(), "application/json".to_string()),
            ],
            body: None,
            timeout: Some(timeout),
            accept_invalid_certs: false,
            digest: None,
        })
    }

    /// Read the validation endpoint's answer. Nothing of its body is read:
    /// it names the token's client, user and scopes.
    pub(crate) fn validated(&mut self, result: Result<HttpResponse, String>) -> Validated {
        self.validating = None;
        let reason = match result {
            Ok(response) if (200..300).contains(&response.status) => {
                self.validate_backoff = RETRY_MIN;
                self.validate_refreshed = false;
                return Validated::Valid;
            }
            Ok(response) if response.status == 401 => {
                self.validate_backoff = RETRY_MIN;
                return Validated::Invalid;
            }
            Ok(response) => format!("HTTP {}", response.status),
            Err(message) => message,
        };
        let wait = self.validate_backoff;
        self.validate_backoff = (self.validate_backoff * 2).min(RETRY_MAX);
        Validated::Failed(reason, wait)
    }

    /// The token endpoint: the `token_url` setting when set, else the spec's.
    pub(crate) fn token_url(&self, settings: &Params) -> Result<String, String> {
        match setting(settings, "token_url") {
            "" => Ok(self.token_url.clone()),
            url => check_token_url(url).map(|()| url.to_string()),
        }
    }

    /// Whether there is a refresh token: without one the access token is a
    /// plain bearer token, used until it is refused.
    pub(crate) fn can_refresh(settings: &Params) -> bool {
        !setting(settings, "refresh_token").is_empty()
    }

    /// Whether the access token must be refreshed before the next request:
    /// there is none, or it expires within the refresh margin.
    pub(crate) fn needs_refresh(&self, settings: &Params, unix: Millis) -> bool {
        if !OAuth::can_refresh(settings) {
            return false;
        }
        if setting(settings, "access_token").is_empty() {
            return true;
        }
        let ahead = match self.lifetime {
            Some(life) => self.refresh_ahead.min(life / 2),
            None => self.refresh_ahead,
        };
        expires_at(settings).is_some_and(|at| (at * 1000) <= unix.saturating_add(ahead))
    }

    /// The refresh request (RFC 6749 §6), form-encoded.
    pub(crate) fn request(
        &self,
        settings: &Params,
        timeout: Millis,
        accept_invalid_certs: bool,
    ) -> Result<HttpRequest, String> {
        let url = self.token_url(settings)?;
        let (id, secret) = (
            setting(settings, "client_id"),
            setting(settings, "client_secret"),
        );
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", setting(settings, "refresh_token")),
        ];
        let mut headers = vec![
            (
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            ),
            ("Accept".to_string(), "application/json".to_string()),
        ];
        match self.client_auth {
            ClientAuth::Body => {
                if !id.is_empty() {
                    form.push(("client_id", id));
                }
                // A public client (an installed app) has no secret, and
                // sends none.
                if !secret.is_empty() {
                    form.push(("client_secret", secret));
                }
            }
            ClientAuth::Basic => {
                let pair = format!("{}:{}", percent_encode(id), percent_encode(secret));
                let encoded = base64::engine::general_purpose::STANDARD.encode(pair);
                headers.push(("Authorization".to_string(), format!("Basic {encoded}")));
            }
        }
        let body = form
            .iter()
            .map(|(k, v)| format!("{k}={}", percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        Ok(HttpRequest {
            method: "POST",
            url,
            headers,
            body: Some(body.into_bytes()),
            timeout: Some(timeout),
            accept_invalid_certs,
            digest: None,
        })
    }

    /// Read the token endpoint's answer. `unix` is the wall clock and `now`
    /// the session's, both in milliseconds.
    pub(crate) fn response(
        &mut self,
        settings: &Params,
        result: Result<HttpResponse, String>,
        unix: Millis,
        now: Millis,
    ) -> Refreshed {
        self.refreshing = None;
        let outcome = match result {
            Err(message) => Refreshed::Failed(message),
            Ok(response) => {
                let doc: Option<Map<String, Value>> = serde_json::from_slice(&response.body).ok();
                let error = doc
                    .as_ref()
                    .and_then(|d| d.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if (200..300).contains(&response.status) && error.is_empty() {
                    match doc.as_ref().and_then(|d| tokens(d, settings, unix)) {
                        Some((tokens, lifetime)) => {
                            self.lifetime = lifetime;
                            Refreshed::Tokens(tokens)
                        }
                        None => Refreshed::Failed(format!(
                            "the token endpoint answered HTTP {} without an access token",
                            response.status
                        )),
                    }
                } else {
                    let described = describe(doc.as_ref(), &error, response.status);
                    if error == "invalid_grant" || matches!(response.status, 400 | 401) {
                        Refreshed::Refused(format!(
                            "the token endpoint refused the refresh ({described})"
                        ))
                    } else {
                        Refreshed::Failed(described)
                    }
                }
            }
        };
        match &outcome {
            Refreshed::Failed(reason) => {
                self.retry_at = now + self.backoff;
                self.backoff = (self.backoff * 2).min(RETRY_MAX);
                self.last_error = Some(reason.clone());
            }
            _ => {
                self.retry_at = 0;
                self.backoff = RETRY_MIN;
                self.last_error = None;
            }
        }
        outcome
    }
}

impl Validate {
    fn new(v: &Value) -> Result<Validate, String> {
        let url = v
            .get("url")
            .and_then(Value::as_str)
            .ok_or("oauth.validate needs a url")?
            .to_string();
        check_token_url(&url)?;
        let every = match v.get("every_s") {
            None => VALIDATE_EVERY_S,
            Some(e) => e
                .as_u64()
                .filter(|&s| s > 0)
                .ok_or("oauth.validate.every_s is a whole number of seconds above 0")?,
        };
        let scheme = match v.get("scheme") {
            None => "Bearer".to_string(),
            Some(s) => s
                .as_str()
                .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()))
                .ok_or("oauth.validate.scheme is a word such as Bearer or OAuth")?
                .to_string(),
        };
        Ok(Validate {
            url,
            every: every * 1000,
            scheme,
        })
    }
}

/// The new settings from a successful answer, and the token's lifetime.
fn tokens(
    doc: &Map<String, Value>,
    settings: &Params,
    unix: Millis,
) -> Option<(Params, Option<Millis>)> {
    let access = doc.get("access_token").and_then(Value::as_str)?;
    if access.is_empty() {
        return None;
    }
    let expires_in = match doc.get("expires_in") {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    };
    let mut out = Params::new();
    out.insert("access_token".into(), Value::from(access));
    out.insert(
        "expires_at".into(),
        expires_in.map_or(Value::Null, |s| Value::from(unix / 1000 + s)),
    );
    // RFC 6749 §6: a new refresh token replaces the old one.
    if let Some(rotated) = doc.get("refresh_token").and_then(Value::as_str) {
        if !rotated.is_empty() && rotated != setting(settings, "refresh_token") {
            out.insert("refresh_token".into(), Value::from(rotated));
        }
    }
    Some((out, expires_in.map(|s| s * 1000)))
}

/// An error answer in words: the OAuth error code and its description
/// (RFC 6749 §5.2), never anything else of the body.
fn describe(doc: Option<&Map<String, Value>>, error: &str, status: u16) -> String {
    let description: String = doc
        .and_then(|d| d.get("error_description"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(DESCRIPTION_MAX)
        .collect();
    match (error.is_empty(), description.is_empty()) {
        (true, _) => format!("HTTP {status}"),
        (false, true) => format!("HTTP {status}, {error}"),
        (false, false) => format!("HTTP {status}, {error}: {description}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn oauth(block: Value) -> OAuth {
        OAuth::new(Some(&block)).unwrap()
    }

    fn google() -> OAuth {
        oauth(json!({"token_url": "https://oauth2.googleapis.com/token"}))
    }

    fn answer(status: u16, body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status,
            body: body.to_string().into_bytes(),
        })
    }

    #[test]
    fn a_refresh_is_due_without_a_token_or_near_expiry() {
        let o = google();
        let base = json!({"refresh_token": "r", "access_token": "a", "expires_at": 10_000});
        // 300 s ahead of expiry at 10,000 s.
        assert!(!o.needs_refresh(&params(base.clone()), 9_699_000));
        assert!(o.needs_refresh(&params(base.clone()), 9_700_000));
        assert!(o.needs_refresh(&params(json!({"refresh_token": "r"})), 0));
        // An unknown expiry is used until refused.
        assert!(!o.needs_refresh(
            &params(json!({"refresh_token": "r", "access_token": "a"})),
            0
        ));
        // A plain bearer token is never refreshed.
        assert!(!o.needs_refresh(
            &params(json!({"access_token": "a", "expires_at": 1})),
            9_999_999
        ));
    }

    #[test]
    fn the_request_is_form_encoded_with_the_client_in_the_body() {
        let o = google();
        let request = o
            .request(
                &params(json!({"refresh_token": "1//r t", "client_id": "id.apps",
                               "client_secret": "s&x"})),
                10_000,
                false,
            )
            .unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://oauth2.googleapis.com/token");
        assert_eq!(
            String::from_utf8(request.body.unwrap()).unwrap(),
            "grant_type=refresh_token&refresh_token=1%2F%2Fr%20t&client_id=id.apps&client_secret=s%26x"
        );
        assert!(request.headers.contains(&(
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into()
        )));
        assert!(!request.headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn a_public_client_sends_no_secret() {
        let request = google()
            .request(
                &params(json!({"refresh_token": "r", "client_id": "id", "client_secret": ""})),
                10_000,
                false,
            )
            .unwrap();
        assert_eq!(
            String::from_utf8(request.body.unwrap()).unwrap(),
            "grant_type=refresh_token&refresh_token=r&client_id=id"
        );
    }

    #[test]
    fn basic_client_authentication_form_encodes_then_base64s() {
        let o = oauth(json!({"token_url": "https://example.com/token", "client_auth": "basic"}));
        let request = o
            .request(
                &params(json!({"refresh_token": "r", "client_id": "a b", "client_secret": "c:d"})),
                1,
                false,
            )
            .unwrap();
        // "a%20b:c%3Ad"
        assert!(request
            .headers
            .contains(&("Authorization".into(), "Basic YSUyMGI6YyUzQWQ=".into())));
        assert_eq!(
            String::from_utf8(request.body.unwrap()).unwrap(),
            "grant_type=refresh_token&refresh_token=r"
        );
    }

    #[test]
    fn the_token_url_setting_overrides_the_spec_and_must_be_safe() {
        let o = google();
        let proxy = params(json!({"token_url": "https://auth.example.com/refresh"}));
        assert_eq!(
            o.token_url(&proxy).unwrap(),
            "https://auth.example.com/refresh"
        );
        assert_eq!(
            o.token_url(&params(json!({"token_url": ""}))).unwrap(),
            "https://oauth2.googleapis.com/token"
        );
        for ok in [
            "http://127.0.0.1:8080/token",
            "http://localhost/token",
            "http://[::1]:9/t",
        ] {
            assert!(check_token_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://192.168.1.2/token",
            "ftp://x",
            "https://",
            "http://localhost.evil.com/",
        ] {
            assert!(check_token_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tokens_are_read_and_a_rotated_refresh_token_reported() {
        let mut o = google();
        let settings = params(json!({"refresh_token": "old"}));
        let got = o.response(
            &settings,
            answer(
                200,
                json!({"access_token": "new", "expires_in": 3599,
                               "token_type": "Bearer", "refresh_token": "rotated"}),
            ),
            1_000_000,
            5,
        );
        assert_eq!(
            got,
            Refreshed::Tokens(params(json!({"access_token": "new", "expires_at": 4599,
                                             "refresh_token": "rotated"})))
        );
        // The same refresh token again is not a rotation.
        let got = o.response(
            &settings,
            answer(
                200,
                json!({"access_token": "n2", "expires_in": "60", "refresh_token": "old"}),
            ),
            0,
            5,
        );
        assert_eq!(
            got,
            Refreshed::Tokens(params(json!({"access_token": "n2", "expires_at": 60})))
        );
        // A 60 s token is refreshed at half its life, not at once.
        let s = params(json!({"refresh_token": "old", "access_token": "n2", "expires_at": 60}));
        assert!(!o.needs_refresh(&s, 29_000));
        assert!(o.needs_refresh(&s, 30_000));
    }

    #[test]
    fn refusals_are_terminal_and_failures_back_off() {
        let mut o = google();
        let s = Params::new();
        let refused = o.response(
            &s,
            answer(400, json!({"error": "invalid_grant", "error_description": "Token has been expired or revoked."})),
            0,
            0,
        );
        assert_eq!(
            refused,
            Refreshed::Refused(
                "the token endpoint refused the refresh (HTTP 400, invalid_grant: Token has been expired or revoked.)".into()
            )
        );
        assert!(matches!(
            o.response(&s, answer(401, json!({"error": "invalid_client"})), 0, 0),
            Refreshed::Refused(_)
        ));
        assert_eq!(
            o.response(&s, Err("connect: refused".into()), 0, 100),
            Refreshed::Failed("connect: refused".into())
        );
        assert_eq!(o.retry_at, 1_100);
        assert!(matches!(
            o.response(&s, answer(503, json!({})), 0, 100),
            Refreshed::Failed(_)
        ));
        assert_eq!(o.retry_at, 2_100);
        assert!(matches!(
            o.response(&s, answer(200, json!({"token_type": "Bearer"})), 0, 0),
            Refreshed::Failed(_)
        ));
        o.response(&s, answer(200, json!({"access_token": "a"})), 0, 0);
        assert_eq!(
            (o.retry_at, o.backoff, o.last_error.clone()),
            (0, RETRY_MIN, None)
        );
    }

    #[test]
    fn validation_sends_the_token_alone_and_reads_only_the_status() {
        let mut o = oauth(json!({"token_url": "https://id.twitch.tv/oauth2/token",
            "validate": {"url": "https://id.twitch.tv/oauth2/validate", "every_s": 3600,
                         "scheme": "OAuth"}}));
        assert_eq!(o.validate.as_ref().unwrap().every, 3_600_000);
        assert!(o.validate_request(&Params::new(), 1).is_none());
        let request = o
            .validate_request(&params(json!({"access_token": "abc", "client_id": "c"})), 7)
            .unwrap();
        assert_eq!(
            (request.method, request.url.as_str()),
            ("GET", "https://id.twitch.tv/oauth2/validate")
        );
        assert_eq!(
            request.headers,
            vec![
                ("Authorization".to_string(), "OAuth abc".to_string()),
                ("Accept".to_string(), "application/json".to_string())
            ]
        );
        assert_eq!(
            o.validated(answer(200, json!({"login": "x"}))),
            Validated::Valid
        );
        assert_eq!(o.validated(answer(401, json!({}))), Validated::Invalid);
        assert_eq!(
            o.validated(answer(503, json!({}))),
            Validated::Failed("HTTP 503".into(), 1_000)
        );
        assert_eq!(
            o.validated(Err("timed out".into())),
            Validated::Failed("timed out".into(), 2_000)
        );
        assert_eq!(o.validated(answer(200, json!({}))), Validated::Valid);
        assert_eq!(o.validate_backoff, RETRY_MIN);
        // Bearer and hourly by default.
        let o = oauth(json!({"token_url": "https://x/t", "validate": {"url": "https://x/v"}}));
        let v = o.validate.as_ref().unwrap();
        assert_eq!((v.every, v.scheme.as_str()), (3_600_000, "Bearer"));
    }

    #[test]
    fn the_validate_block_is_checked() {
        for bad in [
            json!({}),
            json!({"url": "http://example.com/v"}),
            json!({"url": "https://x/v", "every_s": 0}),
            json!({"url": "https://x/v", "scheme": "OAuth x"}),
        ] {
            assert!(
                OAuth::new(Some(&json!({"token_url": "https://x/t", "validate": bad}))).is_err(),
                "{bad}"
            );
        }
        assert!(OAuth::new(Some(
            &json!({"token_url": "https://x/t", "validate": {"url": "http://127.0.0.1:9/v"}})
        ))
        .is_ok());
    }

    #[test]
    fn the_block_is_checked() {
        assert!(OAuth::new(None).is_err());
        assert!(OAuth::new(Some(&json!({}))).is_err());
        assert!(OAuth::new(Some(&json!({"token_url": "http://example.com/t"}))).is_err());
        assert!(OAuth::new(Some(
            &json!({"token_url": "https://x/t", "client_auth": "jwt"})
        ))
        .is_err());
        assert_eq!(
            OAuth::new(Some(
                &json!({"token_url": "https://x/t", "refresh_ahead_s": 60})
            ))
            .unwrap()
            .refresh_ahead,
            60_000
        );
    }
}
