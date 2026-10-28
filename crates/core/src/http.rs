//! HTTP for device modules: one client per trust mode, shared by every session.
//!
//! rustls on every platform, so TLS behaves identically wherever the core runs.
//! TLS 1.2 is the floor: no device the core speaks HTTPS to has been found to
//! need less (a consumer's integration review, item D).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

    /// The request ready to send. A request whose body is streamed from a
    /// file, or whose answer is saved to one (`watch`), has no overall
    /// deadline: its timeout is how long it may go without a byte moving,
    /// so no size is too large (SPEC.md §4, "Timeouts").
    fn build(
        &self,
        request: &HttpRequest,
        watch: Option<&Arc<Progress>>,
    ) -> reqwest::RequestBuilder {
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
                .body(reqwest::Body::wrap_stream(stream_pieces(
                    pieces.clone(),
                    watch.cloned(),
                )));
        }
        if let (Some(ms), None) = (request.timeout, watch) {
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

    /// Make a request whose 2xx answer is saved to the host file `to`
    /// rather than returned: the body is streamed to a file beside it and
    /// moved into place once whole, so a failed download leaves `to` as it
    /// was. The response reported holds no body (SPEC.md §4, "Saving a
    /// response to a file").
    pub(crate) fn spawn_save(
        &self,
        epoch: u64,
        id: RequestId,
        request: &HttpRequest,
        to: PathBuf,
        inbound: mpsc::Sender<Inbound>,
    ) {
        let clients = self.clone();
        let request = request.clone();
        tokio::spawn(async move {
            let result = clients.execute_to(&request, Some(&to)).await;
            let _ = inbound.send(Inbound::Http { epoch, id, result }).await;
        });
    }

    /// Make a request, answering one Digest challenge if it carries
    /// credentials. A second 401 is returned as it is: the engine treats it as
    /// a refusal, so a wrong password costs at most two attempts.
    async fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        self.execute_to(request, None).await
    }

    /// As `execute`, saving a 2xx answer's body to `save` when given.
    async fn execute_to(
        &self,
        request: &HttpRequest,
        save: Option<&Path>,
    ) -> Result<HttpResponse, String> {
        let Some(credentials) = &request.digest else {
            let (response, _) = self.send(request, None, save).await?;
            return Ok(response);
        };
        let (origin, uri) = split_url(&request.url);
        let key = (origin.to_string(), credentials.username.clone());
        let cached = self.answer(&key, credentials, request.method, uri);
        let (response, challenges) = self.send(request, cached, save).await?;
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
        let (response, _) = self.send(request, fresh, save).await?;
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
        save: Option<&Path>,
    ) -> Result<(HttpResponse, Vec<String>), String> {
        // Streamed one way or the other: watched for progress instead of
        // given an overall deadline.
        let idle = Duration::from_millis(request.timeout.unwrap_or(30_000));
        let watch =
            (request.body_stream.is_some() || save.is_some()).then(|| Arc::new(Progress::new()));
        let mut builder = self.build(request, watch.as_ref());
        if let Some(value) = authorization {
            builder = builder.header("Authorization", value);
        }
        let response = match &watch {
            Some(progress) => watched(builder.send(), progress, idle)
                .await?
                .map_err(describe)?,
            None => builder.send().await.map_err(describe)?,
        };
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
        let body = match (&watch, save) {
            (Some(progress), Some(to)) if response.status().is_success() => {
                save_body(response, to, progress, idle).await?;
                Vec::new()
            }
            (Some(progress), _) => watched(response.bytes(), progress, idle)
                .await?
                .map_err(describe)?
                .to_vec(),
            (None, _) => response.bytes().await.map_err(describe)?.to_vec(),
        };
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
        let builder = self.build(request, None);
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

/// How the failure of a transfer that stopped moving starts: the engine
/// reports it as the command's timeout.
pub(crate) const STALLED: &str = "nothing moved for";

/// When a streamed request last moved a byte, either way.
struct Progress {
    start: tokio::time::Instant,
    /// Milliseconds from `start` to the last byte moved.
    last: std::sync::atomic::AtomicU64,
}

impl Progress {
    fn new() -> Progress {
        Progress {
            start: tokio::time::Instant::now(),
            last: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn tick(&self) {
        let ms = self.start.elapsed().as_millis() as u64;
        self.last
            .fetch_max(ms, std::sync::atomic::Ordering::Relaxed);
    }

    fn last(&self) -> tokio::time::Instant {
        self.start + Duration::from_millis(self.last.load(std::sync::atomic::Ordering::Relaxed))
    }
}

/// Run `work` until it ends, failing it once `idle` passes with nothing
/// moved: the timeout of a transfer that may take any time as a whole.
async fn watched<F: std::future::Future>(
    work: F,
    progress: &Progress,
    idle: Duration,
) -> Result<F::Output, String> {
    tokio::pin!(work);
    loop {
        let deadline = progress.last() + idle;
        tokio::select! {
            out = &mut work => return Ok(out),
            _ = tokio::time::sleep_until(deadline) => {
                if progress.last() + idle <= tokio::time::Instant::now() {
                    return Err(format!("{STALLED} {} ms", idle.as_millis()));
                }
            }
        }
    }
}

/// Numbers the partial files of saves in progress, so two saves to one
/// folder never share one.
static PARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Stream a response's body to `to`: written to a file beside it as it
/// arrives (on a blocking thread, a chunk at a time, never held whole), then
/// moved over it once the body has ended. Anything that fails removes the
/// partial file and leaves `to` as it was.
async fn save_body(
    response: reqwest::Response,
    to: &Path,
    progress: &Arc<Progress>,
    idle: Duration,
) -> Result<(), String> {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let part = to.with_file_name(format!(
        ".{name}.{}-{}.part",
        std::process::id(),
        PARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let (chunks, mut queue) = mpsc::channel::<Vec<u8>>(4);
    let writing = {
        let part = part.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::File::create(&part)?;
            while let Some(chunk) = queue.blocking_recv() {
                file.write_all(&chunk)?;
            }
            file.flush()
        })
    };
    let mut body = response.bytes_stream();
    let received = loop {
        match watched(body.next(), progress, idle).await {
            Err(e) => break Err(e),
            Ok(None) => break Ok(()),
            Ok(Some(Err(e))) => break Err(describe(e)),
            Ok(Some(Ok(chunk))) => {
                progress.tick();
                // The writer stopped: its error is reported below.
                if chunks.send(chunk.to_vec()).await.is_err() {
                    break Ok(());
                }
            }
        }
    };
    drop(chunks);
    let written = match writing.await {
        Ok(result) => result.map_err(|e| format!("cannot write {}: {e}", to.display())),
        Err(e) => Err(e.to_string()),
    };
    let moved = received.and(written).and_then(|()| {
        std::fs::rename(&part, to).map_err(|e| format!("cannot write {}: {e}", to.display()))
    });
    if moved.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    moved
}

/// A streamed body's bytes: its pieces in order, each file read a chunk at a
/// time on a blocking thread as the request takes them, so no file is ever
/// held whole. A file that cannot be opened, fails a read or ends before
/// its length fails the request; one that has grown is sent only up to its
/// length. The reading stops when the request is dropped.
fn stream_pieces(
    pieces: Vec<BodyPiece>,
    progress: Option<Arc<Progress>>,
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
    // Each chunk the request takes is progress: an upload is timed out
    // only when the device stops taking it.
    futures_util::stream::unfold(receiver, move |mut receiver| {
        let progress = progress.clone();
        async move {
            let chunk = receiver.recv().await?;
            if let Some(p) = &progress {
                p.tick();
            }
            Some((chunk, receiver))
        }
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

    /// A device on loopback that reads one request's head, then answers
    /// `head`, then each chunk of `body` in turn, then holds the
    /// connection open for `hold` before closing it.
    async fn sender(head: String, body: Vec<Vec<u8>>, hold: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk).await {
                    Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                    _ => return,
                }
            }
            if stream.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            for piece in body {
                if stream.write_all(&piece).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            tokio::time::sleep(hold).await;
            let _ = stream.shutdown().await;
        });
        base
    }

    fn plain_get(url: String, timeout: u64) -> HttpRequest {
        HttpRequest {
            method: "GET",
            url,
            headers: Vec::new(),
            body: None,
            body_stream: None,
            timeout: Some(timeout),
            accept_invalid_certs: false,
            digest: None,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("meros-save-{name}-{}", std::process::id()))
    }

    /// Files left beside `to` by a save: none, once it is done.
    fn parts_beside(to: &Path) -> usize {
        let name = to.file_name().unwrap().to_string_lossy().into_owned();
        std::fs::read_dir(to.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with(&format!(".{name}.")) && n.ends_with(".part")
            })
            .count()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_saved_answer_is_streamed_to_its_file_longer_than_its_timeout() {
        // 3 MiB in chunks over about 0.6 s, against a 300 ms timeout: the
        // timeout counts silence, not the whole transfer.
        let data: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i * 13) as u8).collect();
        let chunks: Vec<Vec<u8>> = data.chunks(100 * 1024).map(<[u8]>::to_vec).collect();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: video/mp4\r\ncontent-length: {}\r\n\r\n",
            data.len()
        );
        let base = sender(head, chunks, Duration::ZERO).await;
        let to = scratch("whole");
        std::fs::write(&to, b"old").unwrap();
        let response = HttpClients::new()
            .unwrap()
            .execute_to(&plain_get(format!("{base}/clip"), 300), Some(&to))
            .await
            .unwrap();
        let saved = std::fs::read(&to).unwrap();
        let _ = std::fs::remove_file(&to);
        assert_eq!(response.status, 200);
        assert!(response.body.is_empty());
        assert_eq!(saved.len(), data.len());
        assert!(saved == data);
        assert_eq!(parts_beside(&to), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_error_answer_is_returned_and_nothing_saved() {
        let base = sender(
            "HTTP/1.1 404 Not Found\r\ncontent-length: 7\r\n\r\nmissing".into(),
            vec![],
            Duration::ZERO,
        )
        .await;
        let to = scratch("error");
        std::fs::write(&to, b"old").unwrap();
        let response = HttpClients::new()
            .unwrap()
            .execute_to(&plain_get(format!("{base}/clip"), 2000), Some(&to))
            .await
            .unwrap();
        let kept = std::fs::read(&to).unwrap();
        let _ = std::fs::remove_file(&to);
        assert_eq!(
            (response.status, response.body.as_slice()),
            (404, &b"missing"[..])
        );
        assert_eq!(kept, b"old");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_save_that_stops_moving_times_out_and_leaves_the_file_as_it_was() {
        let base = sender(
            "HTTP/1.1 200 OK\r\ncontent-length: 1000\r\n\r\n".into(),
            vec![vec![1u8; 100]],
            Duration::from_secs(5),
        )
        .await;
        let to = scratch("stall");
        std::fs::write(&to, b"old").unwrap();
        let result = HttpClients::new()
            .unwrap()
            .execute_to(&plain_get(format!("{base}/clip"), 300), Some(&to))
            .await;
        let kept = std::fs::read(&to).unwrap();
        let _ = std::fs::remove_file(&to);
        assert!(
            result.as_ref().is_err_and(|e| e.starts_with(STALLED)),
            "{result:?}"
        );
        assert_eq!(kept, b"old");
        assert_eq!(parts_beside(&to), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_save_cut_short_fails_and_leaves_no_file() {
        let base = sender(
            "HTTP/1.1 200 OK\r\ncontent-length: 1000\r\n\r\n".into(),
            vec![vec![1u8; 100]],
            Duration::ZERO,
        )
        .await;
        let to = scratch("short");
        let result = HttpClients::new()
            .unwrap()
            .execute_to(&plain_get(format!("{base}/clip"), 2000), Some(&to))
            .await;
        assert!(result.is_err(), "{result:?}");
        assert!(!to.exists());
        assert_eq!(parts_beside(&to), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_upload_waits_for_its_answer_only_as_long_as_its_timeout() {
        // The device takes the body, then says nothing.
        let path = scratch("upload");
        std::fs::write(&path, vec![7u8; 1000]).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
            }
        });
        let mut request = post(
            format!("{base}/upload"),
            vec![BodyPiece::File {
                path: path.clone(),
                len: 1000,
            }],
        );
        request.timeout = Some(300);
        let started = std::time::Instant::now();
        let result = HttpClients::new().unwrap().execute(&request).await;
        let _ = std::fs::remove_file(&path);
        assert!(
            result.as_ref().is_err_and(|e| e.starts_with(STALLED)),
            "{result:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
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
