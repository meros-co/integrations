//! A live preview from any camera that serves its current picture as a JPEG
//! at an HTTP URL (a "snapshot" URL), as most IP and PTZ cameras do.
//!
//! While the `live` stream is watched, the URL is fetched one request at a
//! time, at most every `interval_ms`, and each JPEG is published as a frame.
//! While it is not, the URL is fetched only every 30 s, to report whether the
//! camera is there. A 401 or 403 with credentials configured stops all
//! fetching and reports the camera unauthorized, since retrying credentials on
//! a schedule can lock a device out (as for generic HTTP).
//!
//! A body is a JPEG when it begins with the SOI marker FF D8 (ITU-T T.81
//! Annex B.1.1.3); anything else (a login page, an error page) is not
//! published.

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Credentials, Cx, HttpRequest, HttpResponse, Key, Millis,
    Module, OpenContext, RequestId,
};

/// The stream this module publishes.
pub(crate) const LIVE: Key = "live";
const FETCH: Key = "fetch";
/// How often an unwatched camera is checked.
const CHECK_EVERY: Millis = 30_000;
/// The shortest wait after a failed fetch while watched.
const RETRY_MIN: Millis = 1_000;

pub(crate) struct HttpSnapshot {
    url: String,
    credentials: Option<Credentials>,
    basic: Option<String>,
    accept_invalid_certs: bool,
    timeout: Millis,
    interval: Millis,
    watched: bool,
    /// The request in flight and when it was sent.
    in_flight: Option<(RequestId, Millis)>,
    next_id: RequestId,
    refused: bool,
    last_status: Option<u16>,
    last_error: Option<String>,
}

impl HttpSnapshot {
    pub(crate) fn new(ctx: OpenContext) -> Result<HttpSnapshot, String> {
        let s = &ctx.settings;
        let text = |name: &str| {
            s.get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let scheme = match s.get("scheme").and_then(Value::as_str) {
            None | Some("http") => "http",
            Some("https") => "https",
            Some(other) => return Err(format!("unknown scheme '{other}'")),
        };
        let port = ctx.port.unwrap_or(if scheme == "https" { 443 } else { 80 });
        let host = match ctx.host {
            std::net::IpAddr::V4(ip) => ip.to_string(),
            std::net::IpAddr::V6(ip) => format!("[{ip}]"),
        };
        let path = text("path");
        if !path.starts_with('/') || !path.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("path must begin with '/' and be percent-encoded visible ASCII".into());
        }
        let (username, password) = (text("username"), text("password"));
        let (credentials, basic) = match s.get("auth").and_then(Value::as_str) {
            None | Some("none") => (None, None),
            Some(auth @ ("basic" | "digest")) => {
                let basic = (auth == "basic").then(|| {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD
                        .encode(format!("{username}:{password}"))
                });
                (Some(Credentials { username, password }), basic)
            }
            Some(other) => return Err(format!("unknown auth '{other}'")),
        };
        let number =
            |name: &str, default: Millis| s.get(name).and_then(Value::as_u64).unwrap_or(default);
        Ok(HttpSnapshot {
            url: format!("{scheme}://{host}:{port}{path}"),
            credentials,
            basic,
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            timeout: number("timeout_ms", 5_000),
            interval: number("interval_ms", 200),
            watched: false,
            in_flight: None,
            next_id: 1,
            refused: false,
            last_status: None,
            last_error: None,
        })
    }

    fn fetch(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.refused {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.in_flight = Some((id, cx.now()));
        let headers = self
            .basic
            .iter()
            .map(|token| ("Authorization".to_string(), format!("Basic {token}")))
            .collect();
        cx.http(
            id,
            HttpRequest {
                method: "GET",
                url: self.url.clone(),
                headers,
                body: None,
                timeout: Some(self.timeout),
                accept_invalid_certs: self.accept_invalid_certs,
                digest: self.credentials.clone(),
            },
        );
    }

    fn report(&mut self, cx: &mut Cx, status: Option<u16>, error: Option<String>) {
        let mut patch = serde_json::Map::new();
        if status.is_some() && status != self.last_status {
            patch.insert("status".into(), json!(status));
        }
        if error != self.last_error {
            patch.insert("error".into(), json!(error));
        }
        if status.is_some() {
            self.last_status = status;
        }
        self.last_error = error;
        if !patch.is_empty() {
            cx.state(json!({ "http": patch }));
        }
    }

    fn schedule(&mut self, cx: &mut Cx, sent: Millis, failed: bool) {
        if self.refused {
            return;
        }
        let after = if !self.watched {
            CHECK_EVERY
        } else if failed {
            self.interval.max(RETRY_MIN)
        } else {
            self.interval.saturating_sub(cx.now().saturating_sub(sent))
        };
        cx.set_timer(FETCH, after);
    }
}

fn is_jpeg(body: &[u8]) -> bool {
    body.starts_with(&[0xFF, 0xD8])
}

impl Module for HttpSnapshot {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.fetch(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _params: &Params) {
        cx.complete(
            id,
            Err(CommandError::UnknownCommand {
                command: name.into(),
            }),
        );
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some((current, sent)) = self.in_flight else {
            return;
        };
        if current != id {
            return;
        }
        self.in_flight = None;
        let failed = match result {
            Ok(response) => {
                cx.alive();
                let status = response.status;
                if matches!(status, 401 | 403) && self.credentials.is_some() {
                    self.refused = true;
                    cx.cancel_timer(FETCH);
                    self.report(cx, Some(status), Some(format!("HTTP {status}")));
                    cx.connection(Connection::Unauthorized {
                        reason: format!("HTTP {status}: credentials refused"),
                    });
                    return;
                }
                if !(200..300).contains(&status) {
                    let reason = format!("HTTP {status}");
                    self.report(cx, Some(status), Some(reason.clone()));
                    cx.connection(Connection::Disconnected { reason });
                    true
                } else if !is_jpeg(&response.body) {
                    let reason = "the snapshot URL did not return a JPEG".to_string();
                    self.report(cx, Some(status), Some(reason.clone()));
                    cx.connection(Connection::Disconnected { reason });
                    true
                } else {
                    self.report(cx, Some(status), None);
                    cx.connection(Connection::Connected);
                    if self.watched {
                        cx.frame(LIVE, "jpeg", response.body);
                    }
                    false
                }
            }
            Err(message) => {
                self.report(cx, None, Some(message.clone()));
                cx.connection(Connection::Disconnected { reason: message });
                true
            }
        };
        self.schedule(cx, sent, failed);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == FETCH {
            self.fetch(cx);
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        if stream != LIVE {
            return;
        }
        self.watched = watching;
        if watching {
            cx.cancel_timer(FETCH);
            self.fetch(cx);
        } else if self.in_flight.is_none() && !self.refused {
            cx.set_timer(FETCH, CHECK_EVERY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use serde_json::Map;

    fn module(settings: Value) -> HttpSnapshot {
        let mut s: Map<String, Value> = settings.as_object().unwrap().clone();
        s.entry("path").or_insert(json!("/snapshot.jpg"));
        HttpSnapshot::new(OpenContext {
            host: "10.0.0.5".parse().unwrap(),
            port: None,
            model: "generic".into(),
            channels: None,
            settings: s,
            monitor: true,
        })
        .unwrap()
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((*id, request.clone())),
                _ => None,
            })
            .collect()
    }

    fn jpeg(n: u8) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: vec![0xFF, 0xD8, n, 0xFF, 0xD9],
        }
    }

    #[test]
    fn frames_only_while_watched() {
        let mut m = module(json!({"interval_ms": 100}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let reqs = requests(&cx.take());
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].1.url, "http://10.0.0.5:80/snapshot.jpg");

        // Unwatched: connected, no frame, next check in 30 s.
        let mut cx = Cx::new(10);
        m.http_response(&mut cx, reqs[0].0, Ok(jpeg(1)));
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(!a.iter().any(|a| matches!(a, Action::Frame { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: FETCH,
            after: CHECK_EVERY
        }));

        // Watched: fetch at once, publish, and pace by the interval.
        let mut cx = Cx::new(20);
        m.stream_watch(&mut cx, LIVE, true);
        let reqs = requests(&cx.take());
        assert_eq!(reqs.len(), 1);
        let mut cx = Cx::new(50);
        m.http_response(&mut cx, reqs[0].0, Ok(jpeg(2)));
        let a = cx.take();
        assert!(a.contains(&Action::Frame {
            stream: LIVE,
            format: "jpeg",
            data: vec![0xFF, 0xD8, 2, 0xFF, 0xD9]
        }));
        assert!(a.contains(&Action::SetTimer {
            key: FETCH,
            after: 70
        }));

        // Unwatched again: a fetch in flight finishes without a frame.
        let mut cx = Cx::new(120);
        m.timer(&mut cx, FETCH);
        let reqs = requests(&cx.take());
        let mut cx = Cx::new(121);
        m.stream_watch(&mut cx, LIVE, false);
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(130);
        m.http_response(&mut cx, reqs[0].0, Ok(jpeg(3)));
        let a = cx.take();
        assert!(!a.iter().any(|a| matches!(a, Action::Frame { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: FETCH,
            after: CHECK_EVERY
        }));
    }

    #[test]
    fn a_page_that_is_not_a_jpeg_is_not_published() {
        let mut m = module(json!({}));
        let mut cx = Cx::new(0);
        m.stream_watch(&mut cx, LIVE, true);
        let reqs = requests(&cx.take());
        let mut cx = Cx::new(5);
        m.http_response(
            &mut cx,
            reqs[0].0,
            Ok(HttpResponse {
                status: 200,
                body: b"<html>login</html>".to_vec(),
            }),
        );
        let a = cx.take();
        assert!(!a.iter().any(|a| matches!(a, Action::Frame { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: FETCH,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn refused_credentials_stop_fetching() {
        let mut m = module(json!({"auth": "basic", "username": "admin", "password": "x"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let reqs = requests(&cx.take());
        assert!(reqs[0]
            .1
            .headers
            .iter()
            .any(|(n, v)| n == "Authorization" && v.starts_with("Basic ")));
        let mut cx = Cx::new(5);
        m.http_response(
            &mut cx,
            reqs[0].0,
            Ok(HttpResponse {
                status: 401,
                body: Vec::new(),
            }),
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.iter().any(|a| matches!(a, Action::SetTimer { .. })));
        let mut cx = Cx::new(10);
        m.stream_watch(&mut cx, LIVE, true);
        assert!(requests(&cx.take()).is_empty());
    }
}
