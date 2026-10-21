//! HTTP for device modules: one client per trust mode, shared by every session.
//!
//! rustls on every platform, so TLS behaves identically wherever the core runs.
//! TLS 1.2 is the floor: no device the core speaks HTTPS to has been found to
//! need less (a consumer's integration review, item D).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::digest::{self, Challenge};
use crate::module::{BodyPiece, Credentials, HttpRequest, HttpResponse, Key, RequestId, SseInput};
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
        if let Some(pieces) = &request.body_stream {
            // Sent with its length, so a device that needs Content-Length
            // (most take no chunked upload) gets it; the files are read as
            // the body goes out, a new reading for each attempt.
            let length: u64 = pieces.iter().map(BodyPiece::len).sum();
            builder = builder
                .header(reqwest::header::CONTENT_LENGTH, length)
                .body(reqwest::Body::wrap_stream(stream_pieces(pieces.clone())));
        }
        if let Some(ms) = request.timeout {
            builder = builder.timeout(Duration::from_millis(ms));
        }
        builder
    }

    /// Make a request and report its response to the session.
    pub(crate) fn spawn_request(
        &self,
        epoch: u64,
        id: RequestId,
        request: &HttpRequest,
        inbound: mpsc::Sender<Inbound>,
    ) {
        let clients = self.clone();
        let request = request.clone();
        tokio::spawn(async move {
            let result = clients.execute(&request).await;
            let _ = inbound.send(Inbound::Http { epoch, id, result }).await;
        });
    }

    /// Make a request, answering one Digest challenge if it carries
    /// credentials. A second 401 is returned as it is: the engine treats it as
    /// a refusal, so a wrong password costs at most two attempts.
    async fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        let Some(credentials) = &request.digest else {
            let (response, _) = self.send(request, None).await?;
            return Ok(response);
        };
        let (origin, uri) = split_url(&request.url);
        let key = (origin.to_string(), credentials.username.clone());
        let cached = self.answer(&key, credentials, request.method, uri);
        let (response, challenges) = self.send(request, cached).await?;
        if response.status != 401 {
            return Ok(response);
        }
        let Some(challenge) = digest::choose(challenges.iter().map(String::as_str)) else {
            return Ok(response);
        };
        self.challenges
            .lock()
            .unwrap()
            .insert(key.clone(), (challenge, 0));
        let fresh = self.answer(&key, credentials, request.method, uri);
        let (response, _) = self.send(request, fresh).await?;
        Ok(response)
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

    /// One request: the response, with any `WWW-Authenticate` values.
    async fn send(
        &self,
        request: &HttpRequest,
        authorization: Option<String>,
    ) -> Result<(HttpResponse, Vec<String>), String> {
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
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.as_str().to_ascii_lowercase(), v.to_string()))
            })
            .collect();
        let body = response.bytes().await.map_err(describe)?.to_vec();
        Ok((
            HttpResponse {
                status,
                headers,
                body,
            },
            challenges,
        ))
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
        let json = request
            .headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("accept") && !v.contains("text/event-stream"));
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
            // A stream asked for as JSON (telemetry.sse `format: json`) is
            // split into documents at blank lines, not read as event-stream.
            let mut parser = if json {
                SseParser::json()
            } else {
                SseParser::default()
            };
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

/// How much of a body's file is read at a time.
const FILE_CHUNK: usize = 64 * 1024;

/// A streamed body's bytes: its pieces in order, each file read a chunk at a
/// time on a blocking thread as the request takes them, so no file is ever
/// held whole. A file that cannot be opened, fails a read or ends before
/// its length fails the request; one that has grown is sent only up to its
/// length. The reading stops when the request is dropped.
fn stream_pieces(
    pieces: Vec<BodyPiece>,
) -> impl futures_util::Stream<Item = std::io::Result<Vec<u8>>> + Send + 'static {
    let (chunks, receiver) = mpsc::channel::<std::io::Result<Vec<u8>>>(4);
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        for piece in pieces {
            let (path, len) = match piece {
                BodyPiece::Bytes(bytes) => {
                    if chunks.blocking_send(Ok(bytes)).is_err() {
                        return;
                    }
                    continue;
                }
                BodyPiece::File { path, len } => (path, len),
            };
            let mut file = match std::fs::File::open(&path) {
                Ok(file) => file.take(len),
                Err(e) => {
                    let _ = chunks.blocking_send(Err(e));
                    return;
                }
            };
            let mut sent = 0u64;
            loop {
                let mut chunk = vec![0; FILE_CHUNK];
                match file.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        chunk.truncate(n);
                        sent += n as u64;
                        if chunks.blocking_send(Ok(chunk)).is_err() {
                            return;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        let _ = chunks.blocking_send(Err(e));
                        return;
                    }
                }
            }
            if sent < len {
                let _ = chunks.blocking_send(Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("the file is shorter than the {len} bytes it had when the command was checked"),
                )));
                return;
            }
        }
    });
    futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|chunk| (chunk, receiver))
    })
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
            body_stream: None,
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

    /// A device on loopback that reads one request's head and its
    /// Content-Length body, answers 200 and hands both over.
    async fn receiver() -> (String, tokio::sync::oneshot::Receiver<(String, Vec<u8>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = vec![0u8; 65536];
            let end = loop {
                if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break end;
                }
                match stream.read(&mut chunk).await {
                    Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                    _ => return,
                }
            };
            let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
            let length: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .map_or(0, |v| v.trim().parse().unwrap());
            let mut body = buf.split_off(end + 4);
            while body.len() < length {
                match stream.read(&mut chunk).await {
                    Ok(n) if n > 0 => body.extend_from_slice(&chunk[..n]),
                    _ => return,
                }
            }
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok")
                .await;
            let _ = tx.send((head, body));
        });
        (base, rx)
    }

    fn post(url: String, pieces: Vec<BodyPiece>) -> HttpRequest {
        HttpRequest {
            method: "POST",
            url,
            headers: Vec::new(),
            body: None,
            body_stream: Some(pieces),
            timeout: Some(20_000),
            accept_invalid_certs: false,
            digest: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_streamed_body_reads_its_files_as_it_goes_with_their_length() {
        let data: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i * 7) as u8).collect();
        let path = std::env::temp_dir().join(format!("meros-http-stream-{}", std::process::id()));
        std::fs::write(&path, &data).unwrap();
        let (base, got) = receiver().await;
        let clients = HttpClients::new().unwrap();
        let pieces = vec![
            BodyPiece::Bytes(b"head-".to_vec()),
            BodyPiece::File {
                path: path.clone(),
                len: data.len() as u64,
            },
            BodyPiece::Bytes(b"-tail".to_vec()),
        ];
        let response = clients
            .execute(&post(format!("{base}/upload"), pieces))
            .await;
        let (head, body) = got.await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(response.unwrap().status, 200);
        let total = data.len() + 10;
        assert!(head.contains(&format!("content-length: {total}")), "{head}");
        assert!(!head.contains("transfer-encoding"), "{head}");
        assert_eq!(body.len(), total);
        assert_eq!(&body[..5], b"head-");
        assert_eq!(&body[5..total - 5], &data[..]);
        assert_eq!(&body[total - 5..], b"-tail");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_gone_or_shorter_than_its_length_fails_the_request() {
        let path = std::env::temp_dir().join(format!("meros-http-short-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let clients = HttpClients::new().unwrap();
        let (base, _got) = receiver().await;
        let short = clients
            .execute(&post(
                format!("{base}/upload"),
                vec![BodyPiece::File {
                    path: path.clone(),
                    len: 10,
                }],
            ))
            .await;
        let _ = std::fs::remove_file(&path);
        assert!(short.is_err(), "{short:?}");
        let (base, _got) = receiver().await;
        let gone = clients
            .execute(&post(
                format!("{base}/upload"),
                vec![BodyPiece::File { path, len: 3 }],
            ))
            .await;
        assert!(gone.is_err(), "{gone:?}");
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
