//! HTTP for device modules: one client per trust mode, shared by every session.
//!
//! rustls on every platform, so TLS behaves identically wherever the core runs.
//! TLS 1.2 is the floor: no device the core speaks HTTPS to has been found to
//! need less (RFDeck `docs/INTEGRATIONS_CORE_REVIEW.md`, item D).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::digest::{self, Challenge};
use crate::module::{Credentials, HttpRequest, HttpResponse, Key, RequestId, SseInput};
use crate::session::Inbound;
use crate::sse::SseParser;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// An origin and the username answering its challenge.
type ChallengeKey = (String, String);

#[derive(Clone)]
pub(crate) struct HttpClients {
    strict: reqwest::Client,
    accept_invalid: reqwest::Client,
    /// The last Digest challenge per origin and username, with the count of
    /// requests answered under its nonce.
    challenges: Arc<Mutex<HashMap<ChallengeKey, (Challenge, u32)>>>,
}

impl HttpClients {
    pub(crate) fn new() -> reqwest::Result<HttpClients> {
        let build = |accept_invalid: bool| {
            reqwest::Client::builder()
                .use_rustls_tls()
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .danger_accept_invalid_certs(accept_invalid)
                .connect_timeout(CONNECT_TIMEOUT)
                // SSCv2 requires HTTP/1.1; nothing the core speaks needs h2.
                .http1_only()
                .build()
        };
        Ok(HttpClients {
            strict: build(false)?,
            accept_invalid: build(true)?,
            challenges: Arc::default(),
        })
    }

    fn build(&self, request: &HttpRequest) -> reqwest::RequestBuilder {
        let client = if request.accept_invalid_certs {
            &self.accept_invalid
        } else {
            &self.strict
        };
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        if let Some(ms) = request.timeout {
            builder = builder.timeout(Duration::from_millis(ms));
        }
        builder
    }

    /// Make a request and report its response to the session.
    pub(crate) fn spawn_request(
        &self,
        id: RequestId,
        request: &HttpRequest,
        inbound: mpsc::Sender<Inbound>,
    ) {
        let clients = self.clone();
        let request = request.clone();
        tokio::spawn(async move {
            let result = clients.execute(&request).await;
            let _ = inbound.send(Inbound::Http { id, result }).await;
        });
    }

    /// Make a request, answering one Digest challenge if it carries
    /// credentials. A second 401 is returned as it is: the engine treats it as
    /// a refusal, so a wrong password costs at most two attempts.
    async fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        let Some(credentials) = &request.digest else {
            let (status, _, body) = self.send(request, None).await?;
            return Ok(HttpResponse { status, body });
        };
        let (origin, uri) = split_url(&request.url);
        let key = (origin.to_string(), credentials.username.clone());
        let cached = self.answer(&key, credentials, request.method, uri);
        let (status, challenges, body) = self.send(request, cached).await?;
        if status != 401 {
            return Ok(HttpResponse { status, body });
        }
        let Some(challenge) = digest::choose(challenges.iter().map(String::as_str)) else {
            return Ok(HttpResponse { status, body });
        };
        self.challenges
            .lock()
            .unwrap()
            .insert(key.clone(), (challenge, 0));
        let fresh = self.answer(&key, credentials, request.method, uri);
        let (status, _, body) = self.send(request, fresh).await?;
        Ok(HttpResponse { status, body })
    }

    /// The Authorization header for the cached challenge, counting the use.
    fn answer(
        &self,
        key: &ChallengeKey,
        credentials: &Credentials,
        method: &str,
        uri: &str,
    ) -> Option<String> {
        let mut cache = self.challenges.lock().unwrap();
        let (challenge, nc) = cache.get_mut(key)?;
        *nc += 1;
        Some(digest::authorization(
            challenge,
            &credentials.username,
            &credentials.password,
            method,
            uri,
            *nc,
            &digest::cnonce(),
        ))
    }

    /// One request: status, any `WWW-Authenticate` values, and the body.
    async fn send(
        &self,
        request: &HttpRequest,
        authorization: Option<String>,
    ) -> Result<(u16, Vec<String>, Vec<u8>), String> {
        let mut builder = self.build(request);
        if let Some(value) = authorization {
            builder = builder.header("Authorization", value);
        }
        let response = builder.send().await.map_err(describe)?;
        let status = response.status().as_u16();
        let challenges = response
            .headers()
            .get_all(reqwest::header::WWW_AUTHENTICATE)
            .iter()
            .filter_map(|v| v.to_str().ok().map(String::from))
            .collect();
        let body = response.bytes().await.map_err(describe)?.to_vec();
        Ok((status, challenges, body))
    }

    /// Open a server-sent event stream and report its events to the session.
    pub(crate) fn spawn_stream(
        &self,
        stream: Key,
        generation: u64,
        request: &HttpRequest,
        inbound: mpsc::Sender<Inbound>,
    ) -> tokio::task::JoinHandle<()> {
        let builder = self.build(request);
        tokio::spawn(async move {
            let send = |input: SseInput| {
                let inbound = inbound.clone();
                async move {
                    inbound
                        .send(Inbound::Sse {
                            stream,
                            generation,
                            input,
                        })
                        .await
                        .is_ok()
                }
            };
            let response = match builder.send().await {
                Ok(r) => r,
                Err(e) => {
                    send(SseInput::Closed {
                        status: None,
                        reason: describe(e),
                    })
                    .await;
                    return;
                }
            };
            let status = response.status().as_u16();
            if status != 200 {
                send(SseInput::Closed {
                    status: Some(status),
                    reason: format!("HTTP {status}"),
                })
                .await;
                return;
            }
            if !send(SseInput::Opened).await {
                return;
            }
            let mut parser = SseParser::default();
            let mut body = response.bytes_stream();
            let reason = loop {
                match body.next().await {
                    Some(Ok(chunk)) => {
                        if !send(SseInput::Activity).await {
                            return;
                        }
                        for event in parser.feed(&chunk) {
                            if !send(SseInput::Event(event)).await {
                                return;
                            }
                        }
                    }
                    Some(Err(e)) => break describe(e),
                    None => break "stream ended".to_string(),
                }
            };
            send(SseInput::Closed {
                status: None,
                reason,
            })
            .await;
        })
    }
}

/// `scheme://host:port` and the request target (path and query).
fn split_url(url: &str) -> (&str, &str) {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    match url[after_scheme..].find('/') {
        Some(i) => url.split_at(after_scheme + i),
        None => (url, "/"),
    }
}

fn describe(e: reqwest::Error) -> String {
    use std::error::Error;
    let mut message = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        message.push_str(": ");
        message.push_str(&s.to_string());
        source = s.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use md5::{Digest, Md5};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    fn md5_hex(s: &str) -> String {
        Md5::digest(s.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// The value of `name` in an Authorization header, quoted or not.
    fn param(header: &str, name: &str) -> String {
        let key = format!("{name}=");
        let at = header
            .split(", ")
            .find_map(|p| p.trim_start_matches("Digest ").strip_prefix(&key))
            .unwrap_or("");
        at.trim_matches('"').to_string()
    }

    /// A camera that demands Digest (MD5, qop=auth) for user admin / pass
    /// "s3cret", checked independently of `crate::digest`. Counts its 401s.
    async fn camera() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let refusals = Arc::new(AtomicUsize::new(0));
        let counter = refusals.clone();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 2048];
                    loop {
                        let Ok(n) = stream.read(&mut chunk).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        while let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
                            buf.drain(..end + 4);
                            let target = head.split(' ').nth(1).unwrap_or("").to_string();
                            let auth = head
                                .lines()
                                .find_map(|l| l.strip_prefix("authorization: "))
                                .unwrap_or("");
                            let ha1 = md5_hex("admin:cam:s3cret");
                            let ha2 = md5_hex(&format!("GET:{target}"));
                            let expected = md5_hex(&format!(
                                "{ha1}:n0nce:{}:{}:auth:{ha2}",
                                param(auth, "nc"),
                                param(auth, "cnonce")
                            ));
                            let ok = param(auth, "uri") == target
                                && param(auth, "opaque") == "op"
                                && param(auth, "response") == expected;
                            let reply = if ok {
                                "HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok".to_string()
                            } else {
                                counter.fetch_add(1, Ordering::SeqCst);
                                "HTTP/1.1 401 Unauthorized\r\nwww-authenticate: Digest realm=\"cam\", \
                                 qop=\"auth\", nonce=\"n0nce\", opaque=\"op\"\r\ncontent-length: 0\r\n\r\n"
                                    .to_string()
                            };
                            if stream.write_all(reply.as_bytes()).await.is_err() {
                                return;
                            }
                        }
                    }
                });
            }
        });
        (base, refusals)
    }

    fn get(url: String, password: &str) -> HttpRequest {
        HttpRequest {
            method: "GET",
            url,
            headers: Vec::new(),
            body: None,
            timeout: Some(4000),
            accept_invalid_certs: false,
            digest: Some(Credentials {
                username: "admin".into(),
                password: password.into(),
            }),
        }
    }

    #[tokio::test]
    async fn digest_challenge_is_answered_then_reused() {
        let (base, refusals) = camera().await;
        let clients = HttpClients::new().unwrap();
        let first = clients
            .execute(&get(
                format!("{base}/cgi-bin/param.cgi?get_device_conf"),
                "s3cret",
            ))
            .await
            .unwrap();
        assert_eq!((first.status, first.body.as_slice()), (200, &b"ok"[..]));
        assert_eq!(refusals.load(Ordering::SeqCst), 1, "one challenge");
        // Later requests answer the cached challenge without a new 401.
        for _ in 0..2 {
            let next = clients
                .execute(&get(
                    format!("{base}/cgi-bin/ptzctrl.cgi?ptzcmd&home"),
                    "s3cret",
                ))
                .await
                .unwrap();
            assert_eq!(next.status, 200);
        }
        assert_eq!(refusals.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_wrong_password_is_tried_once_and_the_401_returned() {
        let (base, refusals) = camera().await;
        let clients = HttpClients::new().unwrap();
        let response = clients
            .execute(&get(format!("{base}/"), "wrong"))
            .await
            .unwrap();
        assert_eq!(response.status, 401);
        assert_eq!(refusals.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn urls_split_into_origin_and_target() {
        assert_eq!(
            split_url("http://10.0.0.5:80/cgi-bin/x?a&b"),
            ("http://10.0.0.5:80", "/cgi-bin/x?a&b")
        );
        assert_eq!(split_url("http://[::1]:8080"), ("http://[::1]:8080", "/"));
    }
}
