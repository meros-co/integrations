//! A gRPC client over HTTP/2 ("gRPC over HTTP2", grpc/grpc
//! doc/PROTOCOL-HTTP2.md), sans-IO, for native modules whose device speaks
//! gRPC: unary calls, server-streaming, client-streaming and bidirectional
//! calls, with metadata (headers) per call, on a connection the module
//! opened over plain TCP (HTTP/2 with prior knowledge) or TLS (ALPN `h2`,
//! [`ALPN`]).
//!
//! The module writes [`Client::take_output`] on its connection, feeds what
//! arrives to [`Client::receive`], and gets [`Response`]s back. Messages are
//! bytes: encode and decode them with `crate::protobuf`.
//!
//! - A call is a POST to `/<package>.<Service>/<Method>` with `content-type:
//!   application/grpc`, `te: trailers` and the call's metadata, then its
//!   messages, each a length-prefixed message (one byte, 0 for not
//!   compressed, and a four-byte big-endian length). This side never
//!   compresses, and offers no compression (`grpc-accept-encoding` is not
//!   sent), so a compressed message from the server fails the call.
//! - A call ends with the server's trailers: `grpc-status` (0 for OK) and
//!   `grpc-message`, percent-decoded. A "trailers-only" answer, an HTTP status
//!   other than 200, a reset stream and a GOAWAY that leaves the call
//!   unprocessed all end it with the status the protocol document maps them
//!   to.
//! - Calls beyond the server's SETTINGS_MAX_CONCURRENT_STREAMS wait, in
//!   order, and open as earlier ones end.
//! - Deadlines are the module's: it times a call itself and cancels it
//!   ([`Client::cancel`]), which resets the stream with CANCEL.
//! - [`Client::ping`] sends an HTTP/2 PING, the transport's own liveness
//!   check, which every server answers.
//!
//! Nothing caps a message's size: a message is gathered as it arrives,
//! however long its prefix says it is.

use std::collections::{HashMap, VecDeque};

use crate::http2::{self, Connection, Event};

/// The ALPN protocol a TLS connection for gRPC negotiates.
pub(crate) const ALPN: &str = "h2";

/// gRPC status codes (grpc/grpc doc/statuscodes.md).
pub(crate) mod code {
    pub(crate) const OK: u32 = 0;
    pub(crate) const CANCELLED: u32 = 1;
    pub(crate) const UNKNOWN: u32 = 2;
    pub(crate) const PERMISSION_DENIED: u32 = 7;
    pub(crate) const UNIMPLEMENTED: u32 = 12;
    pub(crate) const INTERNAL: u32 = 13;
    pub(crate) const UNAVAILABLE: u32 = 14;
    pub(crate) const UNAUTHENTICATED: u32 = 16;
}

/// The name of a status code.
pub(crate) fn code_name(code: u32) -> &'static str {
    match code {
        0 => "OK",
        1 => "CANCELLED",
        2 => "UNKNOWN",
        3 => "INVALID_ARGUMENT",
        4 => "DEADLINE_EXCEEDED",
        5 => "NOT_FOUND",
        6 => "ALREADY_EXISTS",
        7 => "PERMISSION_DENIED",
        8 => "RESOURCE_EXHAUSTED",
        9 => "FAILED_PRECONDITION",
        10 => "ABORTED",
        11 => "OUT_OF_RANGE",
        12 => "UNIMPLEMENTED",
        13 => "INTERNAL",
        14 => "UNAVAILABLE",
        15 => "DATA_LOSS",
        16 => "UNAUTHENTICATED",
        _ => "UNKNOWN",
    }
}

/// How a call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub code: u32,
    pub message: String,
}

impl Status {
    pub(crate) fn ok(&self) -> bool {
        self.code == code::OK
    }

    fn new(code: u32, message: impl Into<String>) -> Status {
        Status {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(f, "{}", code_name(self.code))
        } else {
            write!(f, "{}: {}", code_name(self.code), self.message)
        }
    }
}

/// Identifies one call within a client; the client allocates it.
pub(crate) type CallId = u64;

/// What happened on a call, or on the connection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Response {
    /// The connection is up: the server's settings arrived.
    Ready,
    /// The server's response headers (its initial metadata).
    Headers {
        call: CallId,
        metadata: Vec<(String, String)>,
    },
    /// One message, as its bytes.
    Message { call: CallId, message: Vec<u8> },
    /// The call ended. A call's last event.
    Closed { call: CallId, status: Status },
    /// The ack of [`Client::ping`], with its payload.
    Pong([u8; 8]),
    /// The server is closing the connection; calls it did not take are
    /// closed UNAVAILABLE.
    GoingAway { code: u32, debug: String },
}

#[derive(Debug)]
struct Call {
    stream: Option<u32>,
    /// A message being gathered.
    buffer: Vec<u8>,
    /// What waits to be sent before the stream opens: messages, then
    /// whether the sending side closes.
    queued: Vec<Vec<u8>>,
    send_closed: bool,
    headers_seen: bool,
}

/// One call waiting for a stream.
#[derive(Debug)]
struct Waiting {
    call: CallId,
    path: String,
    metadata: Vec<(String, String)>,
}

/// A gRPC client on one HTTP/2 connection.
#[derive(Debug)]
pub(crate) struct Client {
    h2: Connection,
    scheme: &'static str,
    authority: String,
    calls: HashMap<CallId, Call>,
    by_stream: HashMap<u32, CallId>,
    waiting: VecDeque<Waiting>,
    next_call: CallId,
}

impl Client {
    /// A client for a server reached at `authority` (`host:port`), over
    /// `https` (TLS) or `http`.
    pub(crate) fn new(authority: impl Into<String>, tls: bool) -> Client {
        Client {
            h2: Connection::client(),
            scheme: if tls { "https" } else { "http" },
            authority: authority.into(),
            calls: HashMap::new(),
            by_stream: HashMap::new(),
            waiting: VecDeque::new(),
            next_call: 1,
        }
    }

    /// What to write on the connection now.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        self.h2.take_output()
    }

    /// Start a call to `path` (`/package.Service/Method`) with this metadata
    /// (lower-case names). Send its messages with [`Client::send`] and end
    /// its sending side with [`Client::close_send`]; a unary or
    /// server-streaming call sends one message and closes at once
    /// ([`Client::unary`]).
    pub(crate) fn start(&mut self, path: &str, metadata: &[(String, String)]) -> CallId {
        let call = self.next_call;
        self.next_call += 1;
        self.calls.insert(
            call,
            Call {
                stream: None,
                buffer: Vec::new(),
                queued: Vec::new(),
                send_closed: false,
                headers_seen: false,
            },
        );
        self.waiting.push_back(Waiting {
            call,
            path: path.to_string(),
            metadata: metadata.to_vec(),
        });
        self.open_waiting();
        call
    }

    /// Start a call that sends one message and closes its side: a unary or
    /// server-streaming call.
    pub(crate) fn unary(
        &mut self,
        path: &str,
        metadata: &[(String, String)],
        message: Vec<u8>,
    ) -> CallId {
        let call = self.start(path, metadata);
        self.send(call, message);
        self.close_send(call);
        call
    }

    /// Send one message on a call.
    pub(crate) fn send(&mut self, call: CallId, message: Vec<u8>) {
        let Some(c) = self.calls.get_mut(&call) else {
            return;
        };
        if c.send_closed {
            return;
        }
        let framed = frame(&message);
        match c.stream {
            Some(stream) => self.h2.send_data(stream, framed, false),
            None => c.queued.push(framed),
        }
    }

    /// End a call's sending side.
    pub(crate) fn close_send(&mut self, call: CallId) {
        let Some(c) = self.calls.get_mut(&call) else {
            return;
        };
        if c.send_closed {
            return;
        }
        c.send_closed = true;
        if let Some(stream) = c.stream {
            self.h2.send_data(stream, Vec::new(), true);
        }
    }

    /// Cancel a call: its stream is reset with CANCEL, and nothing more is
    /// reported for it.
    pub(crate) fn cancel(&mut self, call: CallId) {
        self.waiting.retain(|w| w.call != call);
        if let Some(c) = self.calls.remove(&call) {
            if let Some(stream) = c.stream {
                self.by_stream.remove(&stream);
                self.h2.reset(stream, http2::CANCEL);
            }
        }
        self.open_waiting();
    }

    /// Whether a call is still open.
    pub(crate) fn is_open(&self, call: CallId) -> bool {
        self.calls.contains_key(&call)
    }

    /// Send an HTTP/2 PING; its ack arrives as [`Response::Pong`].
    pub(crate) fn ping(&mut self, payload: [u8; 8]) {
        self.h2.ping(payload);
    }

    /// Close the connection cleanly (GOAWAY), before closing the socket.
    pub(crate) fn goaway(&mut self) {
        self.h2.goaway();
    }

    fn open_waiting(&mut self) {
        while self.h2.can_open() {
            let Some(w) = self.waiting.pop_front() else {
                break;
            };
            let Some(c) = self.calls.get_mut(&w.call) else {
                continue;
            };
            let mut fields: Vec<(&str, &str)> = vec![
                (":method", "POST"),
                (":scheme", self.scheme),
                (":path", &w.path),
                (":authority", &self.authority),
                ("content-type", "application/grpc"),
                ("te", "trailers"),
            ];
            for (k, v) in &w.metadata {
                fields.push((k, v));
            }
            let end = c.send_closed && c.queued.is_empty();
            let stream = self.h2.open(&fields, end);
            c.stream = Some(stream);
            self.by_stream.insert(stream, w.call);
            let queued = std::mem::take(&mut c.queued);
            let closed = c.send_closed;
            let count = queued.len();
            for (i, m) in queued.into_iter().enumerate() {
                self.h2.send_data(stream, m, closed && i + 1 == count);
            }
        }
    }

    /// Feed bytes received on the connection. An error is a connection
    /// error: close the connection and fail its calls.
    pub(crate) fn receive(&mut self, bytes: &[u8]) -> Result<Vec<Response>, String> {
        let events = self.h2.receive(bytes)?;
        let mut out = Vec::new();
        for event in events {
            match event {
                Event::Ready => out.push(Response::Ready),
                Event::Pong(p) => out.push(Response::Pong(p)),
                Event::GoAway {
                    last_stream,
                    code,
                    debug,
                } => {
                    let gone: Vec<u32> = self
                        .by_stream
                        .keys()
                        .copied()
                        .filter(|s| *s > last_stream)
                        .collect();
                    for stream in gone {
                        if let Some(call) = self.by_stream.remove(&stream) {
                            self.calls.remove(&call);
                            out.push(Response::Closed {
                                call,
                                status: Status::new(code::UNAVAILABLE, "the server is going away"),
                            });
                        }
                    }
                    for w in std::mem::take(&mut self.waiting) {
                        self.calls.remove(&w.call);
                        out.push(Response::Closed {
                            call: w.call,
                            status: Status::new(code::UNAVAILABLE, "the server is going away"),
                        });
                    }
                    out.push(Response::GoingAway { code, debug });
                }
                Event::Reset {
                    stream,
                    code: reason,
                } => {
                    if let Some(call) = self.by_stream.remove(&stream) {
                        self.calls.remove(&call);
                        let status = match reason {
                            http2::CANCEL => {
                                Status::new(code::CANCELLED, "the server cancelled the call")
                            }
                            0x7 => Status::new(code::UNAVAILABLE, "the server refused the stream"),
                            other => Status::new(
                                code::INTERNAL,
                                format!("the server reset the stream (HTTP/2 error {other})"),
                            ),
                        };
                        out.push(Response::Closed { call, status });
                    }
                }
                Event::Headers {
                    stream,
                    fields,
                    end_stream,
                } => {
                    let Some(&call) = self.by_stream.get(&stream) else {
                        continue;
                    };
                    let first = self.calls.get(&call).is_some_and(|c| !c.headers_seen);
                    if first {
                        if let Some(c) = self.calls.get_mut(&call) {
                            c.headers_seen = true;
                        }
                        let status = fields
                            .iter()
                            .find(|(k, _)| k == ":status")
                            .map(|(_, v)| v.as_str());
                        if status != Some("200") {
                            let status = http_status(status);
                            self.finish(call, stream, status, &mut out);
                            continue;
                        }
                        let content_type = fields
                            .iter()
                            .find(|(k, _)| k == "content-type")
                            .map(|(_, v)| v.as_str());
                        if !content_type.is_some_and(|t| t.starts_with("application/grpc")) {
                            let status = Status::new(
                                code::UNKNOWN,
                                format!(
                                    "the server answered with content-type {}",
                                    content_type.unwrap_or("none")
                                ),
                            );
                            self.finish(call, stream, status, &mut out);
                            continue;
                        }
                        if !end_stream {
                            out.push(Response::Headers {
                                call,
                                metadata: fields
                                    .into_iter()
                                    .filter(|(k, _)| !k.starts_with(':'))
                                    .collect(),
                            });
                            continue;
                        }
                    }
                    // Trailers, or a trailers-only answer.
                    let status = trailers_status(&fields);
                    let leftover = self.calls.get(&call).is_some_and(|c| !c.buffer.is_empty());
                    let status = if leftover && status.ok() {
                        Status::new(code::INTERNAL, "the call ended within a message")
                    } else {
                        status
                    };
                    self.finish(call, stream, status, &mut out);
                }
                Event::Data {
                    stream,
                    data,
                    end_stream,
                } => {
                    let Some(&call) = self.by_stream.get(&stream) else {
                        continue;
                    };
                    let mut failure = None;
                    if let Some(c) = self.calls.get_mut(&call) {
                        c.buffer.extend_from_slice(&data);
                        while c.buffer.len() >= 5 {
                            let len = u32::from_be_bytes([
                                c.buffer[1],
                                c.buffer[2],
                                c.buffer[3],
                                c.buffer[4],
                            ]) as usize;
                            if c.buffer.len() < 5 + len {
                                break;
                            }
                            if c.buffer[0] != 0 {
                                failure = Some(Status::new(code::INTERNAL, "the server sent a compressed message, which this side did not offer"));
                                break;
                            }
                            let message = c.buffer[5..5 + len].to_vec();
                            c.buffer.drain(..5 + len);
                            out.push(Response::Message { call, message });
                        }
                    }
                    if let Some(status) = failure {
                        self.cancel(call);
                        out.push(Response::Closed { call, status });
                    } else if end_stream {
                        // The stream ended without trailers.
                        self.finish(
                            call,
                            stream,
                            Status::new(
                                code::INTERNAL,
                                "the server ended the call without a status",
                            ),
                            &mut out,
                        );
                    }
                }
            }
        }
        self.open_waiting();
        Ok(out)
    }

    fn finish(&mut self, call: CallId, stream: u32, status: Status, out: &mut Vec<Response>) {
        self.by_stream.remove(&stream);
        if self.calls.remove(&call).is_some() {
            // Whatever this side has not sent is no longer wanted.
            self.h2.reset(stream, http2::NO_ERROR);
            out.push(Response::Closed { call, status });
        }
    }

    /// Fail every call: the connection is gone. Returns their closing.
    pub(crate) fn fail_all(&mut self, message: &str) -> Vec<Response> {
        let mut out: Vec<Response> = self
            .calls
            .keys()
            .map(|call| Response::Closed {
                call: *call,
                status: Status::new(code::UNAVAILABLE, message),
            })
            .collect();
        out.sort_by_key(|r| match r {
            Response::Closed { call, .. } => *call,
            _ => 0,
        });
        self.calls.clear();
        self.by_stream.clear();
        self.waiting.clear();
        out
    }
}

/// A length-prefixed message, not compressed.
pub(crate) fn frame(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + message.len());
    out.push(0);
    out.extend_from_slice(&(message.len() as u32).to_be_bytes());
    out.extend_from_slice(message);
    out
}

/// The status in trailers: `grpc-status`, and `grpc-message` percent-decoded.
fn trailers_status(fields: &[(String, String)]) -> Status {
    let value = |name: &str| {
        fields
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    match value("grpc-status").and_then(|s| s.trim().parse::<u32>().ok()) {
        Some(c) => Status::new(c, percent_decode(value("grpc-message").unwrap_or(""))),
        None => Status::new(code::UNKNOWN, "the server sent no grpc-status"),
    }
}

/// An HTTP status other than 200, mapped as the protocol document says
/// ("HTTP to gRPC Status Code Mapping").
fn http_status(status: Option<&str>) -> Status {
    let text = status.unwrap_or("none");
    let c = match status {
        Some("400") => code::INTERNAL,
        Some("401") => code::UNAUTHENTICATED,
        Some("403") => code::PERMISSION_DENIED,
        Some("404") => code::UNIMPLEMENTED,
        Some("429" | "502" | "503" | "504") => code::UNAVAILABLE,
        _ => code::UNKNOWN,
    };
    Status::new(c, format!("HTTP status {text}"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A gRPC server's side of one connection, for tests standing in for a
/// device: it reads calls and answers them.
#[cfg(test)]
pub(crate) mod server {
    use super::*;

    /// What a client did.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) enum Request {
        Start {
            stream: u32,
            path: String,
            metadata: Vec<(String, String)>,
        },
        Message {
            stream: u32,
            message: Vec<u8>,
        },
        HalfClose {
            stream: u32,
        },
        Cancel {
            stream: u32,
        },
        Ping,
    }

    pub(crate) struct Server {
        pub h2: Connection,
        buffers: HashMap<u32, Vec<u8>>,
        started: std::collections::HashSet<u32>,
    }

    impl Server {
        pub(crate) fn new() -> Server {
            Server {
                h2: Connection::server(),
                buffers: HashMap::new(),
                started: Default::default(),
            }
        }

        pub(crate) fn receive(&mut self, bytes: &[u8]) -> Result<Vec<Request>, String> {
            let mut out = Vec::new();
            for event in self.h2.receive(bytes)? {
                match event {
                    Event::Headers {
                        stream,
                        fields,
                        end_stream,
                    } => {
                        if self.started.insert(stream) {
                            let path = fields
                                .iter()
                                .find(|(k, _)| k == ":path")
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default();
                            out.push(Request::Start {
                                stream,
                                path,
                                metadata: fields
                                    .into_iter()
                                    .filter(|(k, _)| !k.starts_with(':'))
                                    .collect(),
                            });
                        }
                        if end_stream {
                            out.push(Request::HalfClose { stream });
                        }
                    }
                    Event::Data {
                        stream,
                        data,
                        end_stream,
                    } => {
                        let buf = self.buffers.entry(stream).or_default();
                        buf.extend_from_slice(&data);
                        while buf.len() >= 5 {
                            let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
                            if buf.len() < 5 + len {
                                break;
                            }
                            out.push(Request::Message {
                                stream,
                                message: buf[5..5 + len].to_vec(),
                            });
                            buf.drain(..5 + len);
                        }
                        if end_stream {
                            out.push(Request::HalfClose { stream });
                        }
                    }
                    Event::Reset { stream, .. } => out.push(Request::Cancel { stream }),
                    Event::Ready | Event::Pong(_) | Event::GoAway { .. } => {}
                }
            }
            Ok(out)
        }

        /// Answer a call's start: HTTP 200 and the gRPC content type.
        pub(crate) fn headers(&mut self, stream: u32) {
            self.h2.send_headers(
                stream,
                &[(":status", "200"), ("content-type", "application/grpc")],
                false,
            );
        }

        pub(crate) fn message(&mut self, stream: u32, message: &[u8]) {
            self.h2.send_data(stream, frame(message), false);
        }

        pub(crate) fn close(&mut self, stream: u32, status: u32, message: &str) {
            let status = status.to_string();
            self.h2.send_headers(
                stream,
                &[("grpc-status", &status), ("grpc-message", message)],
                true,
            );
        }

        pub(crate) fn take_output(&mut self) -> Vec<u8> {
            self.h2.take_output()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::server::{Request, Server};
    use super::*;

    fn pump_to_server(c: &mut Client, s: &mut Server) -> Vec<Request> {
        s.receive(&c.take_output()).unwrap()
    }

    fn pump_to_client(s: &mut Server, c: &mut Client) -> Vec<Response> {
        c.receive(&s.take_output()).unwrap()
    }

    fn connected() -> (Client, Server) {
        let mut c = Client::new("10.0.0.1:17475", false);
        let mut s = Server::new();
        pump_to_server(&mut c, &mut s);
        assert_eq!(pump_to_client(&mut s, &mut c), [Response::Ready]);
        pump_to_server(&mut c, &mut s);
        (c, s)
    }

    #[test]
    fn a_unary_call_carries_its_path_metadata_and_message() {
        let (mut c, mut s) = connected();
        let call = c.unary(
            "/pkg.Svc/Get",
            &[("sessionid".into(), "abc".into())],
            b"\x08\x01".to_vec(),
        );
        let requests = pump_to_server(&mut c, &mut s);
        let Request::Start {
            stream,
            path,
            metadata,
        } = &requests[0]
        else {
            panic!("{requests:?}")
        };
        assert_eq!(path, "/pkg.Svc/Get");
        assert_eq!(
            metadata,
            &[
                ("content-type".to_string(), "application/grpc".to_string()),
                ("te".into(), "trailers".into()),
                ("sessionid".into(), "abc".into())
            ]
        );
        assert_eq!(
            requests[1],
            Request::Message {
                stream: *stream,
                message: vec![8, 1]
            }
        );
        assert_eq!(requests[2], Request::HalfClose { stream: *stream });
        s.headers(*stream);
        s.message(*stream, b"\x0a\x00");
        s.close(*stream, 0, "");
        let responses = pump_to_client(&mut s, &mut c);
        assert_eq!(
            responses,
            [
                Response::Headers {
                    call,
                    metadata: vec![("content-type".into(), "application/grpc".into())]
                },
                Response::Message {
                    call,
                    message: vec![0x0a, 0]
                },
                Response::Closed {
                    call,
                    status: Status {
                        code: 0,
                        message: String::new()
                    }
                },
            ]
        );
        assert!(!c.is_open(call));
    }

    #[test]
    fn the_message_prefix_is_one_flag_byte_and_a_big_endian_length() {
        assert_eq!(frame(b"abc"), [0, 0, 0, 0, 3, b'a', b'b', b'c']);
    }

    #[test]
    fn statuses_come_from_trailers_http_and_resets() {
        let (mut c, mut s) = connected();
        let call = c.unary("/pkg.Svc/Fail", &[], vec![]);
        let stream = match &pump_to_server(&mut c, &mut s)[0] {
            Request::Start { stream, .. } => *stream,
            other => panic!("{other:?}"),
        };
        // Trailers-only, with a percent-encoded message.
        s.h2.send_headers(
            stream,
            &[
                (":status", "200"),
                ("content-type", "application/grpc"),
                ("grpc-status", "7"),
                ("grpc-message", "no%20way"),
            ],
            true,
        );
        assert_eq!(
            pump_to_client(&mut s, &mut c),
            [Response::Closed {
                call,
                status: Status {
                    code: code::PERMISSION_DENIED,
                    message: "no way".into()
                }
            }]
        );
        let call = c.unary("/pkg.Svc/Missing", &[], vec![]);
        let stream = match &pump_to_server(&mut c, &mut s)[0] {
            Request::Start { stream, .. } => *stream,
            other => panic!("{other:?}"),
        };
        s.h2.send_headers(stream, &[(":status", "404")], true);
        assert_eq!(
            pump_to_client(&mut s, &mut c),
            [Response::Closed {
                call,
                status: Status {
                    code: code::UNIMPLEMENTED,
                    message: "HTTP status 404".into()
                }
            }]
        );
        let call = c.unary("/pkg.Svc/Reset", &[], vec![]);
        let stream = match &pump_to_server(&mut c, &mut s)[0] {
            Request::Start { stream, .. } => *stream,
            other => panic!("{other:?}"),
        };
        s.h2.reset(stream, http2::CANCEL);
        // The test server forgot the stream; reset it on the wire anyway.
        let mut bytes = vec![0, 0, 4, 3, 0];
        bytes.extend_from_slice(&stream.to_be_bytes());
        bytes.extend_from_slice(&8u32.to_be_bytes());
        assert_eq!(
            c.receive(&bytes).unwrap(),
            [Response::Closed {
                call,
                status: Status {
                    code: code::CANCELLED,
                    message: "the server cancelled the call".into()
                }
            }]
        );
    }

    #[test]
    fn a_bidirectional_call_streams_both_ways_and_messages_may_span_frames() {
        let (mut c, mut s) = connected();
        let call = c.start("/pkg.Svc/Ping", &[]);
        c.send(call, b"one".to_vec());
        let requests = pump_to_server(&mut c, &mut s);
        let Request::Start { stream, .. } = requests[0] else {
            panic!()
        };
        assert_eq!(
            requests[1],
            Request::Message {
                stream,
                message: b"one".to_vec()
            }
        );
        s.headers(stream);
        // A message split over two DATA frames.
        let framed = frame(b"pong");
        s.h2.send_data(stream, framed[..3].to_vec(), false);
        let first = pump_to_client(&mut s, &mut c);
        assert_eq!(first.len(), 1, "headers only: {first:?}");
        s.h2.send_data(stream, framed[3..].to_vec(), false);
        assert_eq!(
            pump_to_client(&mut s, &mut c),
            [Response::Message {
                call,
                message: b"pong".to_vec()
            }]
        );
        c.send(call, b"two".to_vec());
        c.close_send(call);
        let requests = pump_to_server(&mut c, &mut s);
        assert_eq!(
            requests,
            [
                Request::Message {
                    stream,
                    message: b"two".to_vec()
                },
                Request::HalfClose { stream }
            ]
        );
        c.cancel(call);
        assert_eq!(pump_to_server(&mut c, &mut s), [Request::Cancel { stream }]);
    }

    #[test]
    fn calls_beyond_the_servers_limit_wait_their_turn() {
        let mut c = Client::new("h:1", false);
        let mut s = Server::new();
        pump_to_server(&mut c, &mut s);
        // Calls started before the server's settings wait too.
        let a = c.unary("/p.S/A", &[], vec![]);
        // SETTINGS_MAX_CONCURRENT_STREAMS 1.
        let mut settings = vec![0, 0, 6, 4, 0, 0, 0, 0, 0];
        settings.extend_from_slice(&[0, 3, 0, 0, 0, 1]);
        s.take_output();
        assert_eq!(c.receive(&settings).unwrap(), [Response::Ready]);
        let b = c.unary("/p.S/B", &[], vec![]);
        let requests = pump_to_server(&mut c, &mut s);
        let starts: Vec<&Request> = requests
            .iter()
            .filter(|r| matches!(r, Request::Start { .. }))
            .collect();
        assert_eq!(starts.len(), 1, "one at a time");
        let Request::Start { stream, path, .. } = starts[0] else {
            panic!()
        };
        assert_eq!(path, "/p.S/A");
        s.headers(*stream);
        s.close(*stream, 0, "");
        let responses = pump_to_client(&mut s, &mut c);
        assert!(responses.contains(&Response::Closed {
            call: a,
            status: Status {
                code: 0,
                message: String::new()
            }
        }));
        let requests = pump_to_server(&mut c, &mut s);
        assert!(matches!(&requests[0], Request::Start { path, .. } if path == "/p.S/B"));
        assert!(c.is_open(b));
        assert_eq!(
            c.fail_all("gone"),
            [Response::Closed {
                call: b,
                status: Status {
                    code: code::UNAVAILABLE,
                    message: "gone".into()
                }
            }]
        );
    }

    #[test]
    fn a_compressed_message_fails_the_call() {
        let (mut c, mut s) = connected();
        let call = c.unary("/p.S/A", &[], vec![]);
        let Request::Start { stream, .. } = pump_to_server(&mut c, &mut s)[0].clone() else {
            panic!()
        };
        s.headers(stream);
        s.h2.send_data(stream, vec![1, 0, 0, 0, 1, 0], false);
        let responses = pump_to_client(&mut s, &mut c);
        assert!(
            matches!(responses.last(), Some(Response::Closed { call: x, status }) if *x == call && status.code == code::INTERNAL)
        );
    }
}
