//! The interface every device module implements.
//!
//! A module is protocol logic only. It never opens a socket or reads a clock:
//! it is handed commands, inbound data and timer expiries, and answers with
//! [`Action`]s that the session carries out. That keeps a module deterministic,
//! so it can be tested by feeding it recorded bytes and synthetic time, and it
//! means no module can differ from another in how it uses the network.

use std::net::SocketAddr;

use serde::Serialize;
use serde_json::Value;

use crate::catalog::Params;

/// Milliseconds since the session started. Monotonic, and synthetic in tests.
pub type Millis = u64;

/// Identifies one pending command within a session.
pub type CommandId = u64;

/// A module-chosen name for a socket, timer, request or stream. Modules use
/// small constants; the session maps them to real resources.
pub type Key = &'static str;

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// The device confirmed the command.
    Ack,
    /// The device returned a value.
    Value { value: Value },
    /// Sent, but the protocol gives no way to confirm it was applied.
    Unverified,
}

/// Why a command failed. The set is closed and identical in every delivery.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CommandError {
    #[error("invalid parameters: {message}")]
    InvalidParams { message: String },
    #[error("unknown command '{command}'")]
    UnknownCommand { command: String },
    #[error("'{command}' is not supported by model '{model}'")]
    UnsupportedForModel { command: String, model: String },
    #[error("device is not connected")]
    NotConnected,
    #[error("no reply within the timeout")]
    Timeout,
    #[error("device rejected the command: {message}")]
    DeviceError {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        message: String,
    },
    #[error("transport error: {message}")]
    Transport { message: String },
    #[error("authentication failed: {message}")]
    Auth { message: String },
    #[error("session closed")]
    Closed,
}

pub type CommandResult = Result<Outcome, CommandError>;

/// Identifies one HTTP request within a module; the module allocates it.
pub type RequestId = u64;

/// An HTTP request a module asks the session to make.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Overall deadline for the response. `None` only for streams.
    pub timeout: Option<Millis>,
    /// Accept a certificate that does not chain to a trusted root, such as a
    /// device's self-signed one. The connection is still encrypted; the peer is
    /// not authenticated.
    pub accept_invalid_certs: bool,
    /// Answer a Digest challenge (RFC 7616) with these credentials. The
    /// session retries the request once with the answer and reuses the
    /// challenge for later requests to the same origin.
    pub digest: Option<Credentials>,
}

/// A TCP stream opened through an SSH tunnel (`direct-tcpip`), for devices
/// that accept their control protocol only over SSH port forwarding.
#[derive(Clone, PartialEq)]
pub struct SshTunnel {
    /// The device's SSH server.
    pub ssh: SocketAddr,
    pub username: String,
    pub password: String,
    /// The host key's SHA-256 fingerprint as OpenSSH prints it
    /// (`SHA256:...`). Without it the device is not verified.
    pub fingerprint: Option<String>,
    /// A cipher the device requires, offered first (Sony: `aes128-ctr`).
    pub cipher: Option<String>,
    /// Where the device forwards the stream, as it sees it
    /// (Sony: `localhost:15740`).
    pub target_host: String,
    pub target_port: u16,
}

impl std::fmt::Debug for SshTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshTunnel")
            .field("ssh", &self.ssh)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("fingerprint", &self.fingerprint)
            .field("cipher", &self.cipher)
            .field("target_host", &self.target_host)
            .field("target_port", &self.target_port)
            .finish()
    }
}

/// A TLS stream a module asks the session to open.
#[derive(Debug, Clone, PartialEq)]
pub struct TlsTarget {
    pub to: SocketAddr,
    /// The name the certificate is checked against: the device's host name,
    /// or its address as text.
    pub server_name: String,
    /// Accept a certificate that does not chain to a trusted root or does not
    /// name the device, such as a device's self-signed one. The stream is
    /// still encrypted; the device is not authenticated.
    pub accept_invalid_certs: bool,
}

/// A username and password.
#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// What happens on a server-sent event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum SseInput {
    /// The server accepted the request and the stream is open.
    Opened,
    Event(crate::sse::SseEvent),
    /// Bytes arrived, whether or not they completed an event. Keepalive
    /// comments are proof of life too.
    Activity,
    /// The stream ended. `status` is set when the server refused to open it.
    Closed {
        status: Option<u16>,
        reason: String,
    },
}

/// What happens to a file a module writes on the host.
#[derive(Debug, Clone, PartialEq)]
pub enum FileInput {
    /// A file opened with `file_open_append` is open; `bytes` is its length
    /// before anything is appended.
    Opened { bytes: u64 },
    /// Every chunk was written and the file closed. `bytes` is the file's
    /// length: what was written, plus, when appending, what was there before.
    Closed { bytes: u64 },
    /// The file could not be created, written or read. Reported once; later
    /// writes are dropped.
    Failed { message: String },
    /// A file asked for with `file_read`, whole.
    Read { data: Vec<u8> },
}

/// What happens on a TCP connection.
#[derive(Debug, Clone, PartialEq)]
pub enum TcpInput {
    Connected,
    Data(Vec<u8>),
    /// The connection failed or ended. It is gone; open it again to retry.
    Closed {
        reason: String,
    },
}

/// A WebSocket a module asks the session to open.
#[derive(Debug, Clone, PartialEq)]
pub struct WsRequest {
    /// `ws://host:port/path` or `wss://host:port/path`.
    pub url: String,
    /// Extra handshake headers, such as `Sec-WebSocket-Protocol`.
    pub headers: Vec<(String, String)>,
    /// Over `wss`, accept a certificate that does not chain to a trusted
    /// root, such as a device's self-signed one. Encrypted, not authenticated.
    pub accept_invalid_certs: bool,
}

/// What happens on a WebSocket.
#[derive(Debug, Clone, PartialEq)]
pub enum WsInput {
    Opened,
    Text(String),
    Binary(Vec<u8>),
    /// A ping or pong arrived: proof of life, nothing to handle.
    Activity,
    /// The connection failed or ended. `code` is the close code the peer sent,
    /// if it sent one. It is gone; open it again to retry.
    Closed {
        code: Option<u16>,
        reason: String,
    },
}

/// Where a UDP socket binds locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bind {
    /// An ephemeral port owned by this session. The device replies to it.
    Ephemeral,
    /// A fixed port shared by every session that asks for it, with inbound
    /// datagrams routed by source address. For protocols whose devices reply to
    /// a fixed port rather than the sender's (Sennheiser MCP on 53212).
    Shared(u16),
}

/// Whether the device is reachable, as the module judges it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Connection {
    Connecting,
    Connected,
    Disconnected {
        reason: String,
    },
    /// Reachable, but the device refused the configured credentials.
    Unauthorized {
        reason: String,
    },
    /// The protocol offers no way to tell whether the device is there: it never
    /// replies. Commands are sent and reported `unverified`.
    Unmonitored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warning,
}

/// Something a module asks the session to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    UdpOpen {
        socket: Key,
        bind: Bind,
    },
    UdpSend {
        socket: Key,
        to: SocketAddr,
        data: Vec<u8>,
    },
    UdpClose {
        socket: Key,
    },
    /// Connect, replacing any connection open under this key.
    TcpOpen {
        socket: Key,
        to: SocketAddr,
    },
    /// Connect through an SSH tunnel, replacing any connection open under
    /// this key. Reported like `TcpOpen`; a refused login or a host key that
    /// does not match closes it with a reason starting `ssh refused:`.
    TcpOpenSsh {
        socket: Key,
        tunnel: SshTunnel,
    },
    /// Connect and negotiate TLS (1.2 or later), replacing any connection
    /// open under this key. Reported like `TcpOpen` once the handshake is
    /// done; a failed handshake, including a certificate that is not trusted,
    /// closes it with a reason starting `tls:`.
    TcpOpenTls {
        socket: Key,
        target: TlsTarget,
    },
    TcpSend {
        socket: Key,
        data: Vec<u8>,
    },
    /// Create (or truncate) a file on the host, for a download too large to
    /// return as a value. The path is the consumer's, passed through.
    FileOpen {
        file: Key,
        path: String,
    },
    /// Open an existing file on the host for appending, for a download that
    /// resumes. Reported as `FileInput::Opened` with the file's length, then
    /// as for `FileOpen`; a missing or unwritable file is `Failed`.
    FileAppend {
        file: Key,
        path: String,
    },
    /// Append to an open file.
    FileWrite {
        file: Key,
        data: Vec<u8>,
    },
    /// Close the file; reported as `FileInput::Closed` once written.
    FileClose {
        file: Key,
    },
    /// Read a whole file on the host, for an upload (a LUT, a scene file).
    /// Reported as `FileInput::Read`, or `Failed` for a missing or unreadable
    /// file or one larger than `max_bytes`.
    FileRead {
        file: Key,
        path: String,
        max_bytes: u64,
    },
    TcpClose {
        socket: Key,
    },
    /// Accept connections from the device on a local TCP port, shared with
    /// other sessions and routed by peer address. Each accepted connection is
    /// reported on `socket` as `TcpInput::Connected`, replacing any before it.
    TcpListen {
        socket: Key,
        port: u16,
    },
    /// Open a WebSocket, replacing any open under this key.
    WsOpen {
        socket: Key,
        request: WsRequest,
    },
    WsSend {
        socket: Key,
        text: String,
    },
    WsClose {
        socket: Key,
    },
    Http {
        id: RequestId,
        request: HttpRequest,
    },
    /// Open a server-sent event stream, replacing any open under this key.
    SseOpen {
        stream: Key,
        request: HttpRequest,
    },
    SseClose {
        stream: Key,
    },
    /// Fire [`Module::timer`] with this key after `after` ms, replacing any
    /// pending timer with the same key.
    SetTimer {
        key: Key,
        after: Millis,
    },
    CancelTimer {
        key: Key,
    },
    Complete {
        id: CommandId,
        result: CommandResult,
    },
    /// Merge into the device state (RFC 7386 JSON merge patch).
    State(Value),
    Connection(Connection),
    /// Heard from the device, whether or not it carried telemetry.
    Alive,
    /// How long the device took to answer a request just answered, in
    /// milliseconds: the time from sending it to its reply.
    RoundTrip(Millis),
    Log {
        level: Level,
        message: String,
    },
    /// Publish one frame of a stream the spec declares, such as a camera's
    /// live view. Frames bypass the event queue: each watcher keeps only the
    /// newest, and a frame nobody watches is discarded.
    Frame {
        stream: Key,
        format: &'static str,
        data: Vec<u8>,
    },
}

/// Collects a module's actions for one callback.
#[derive(Debug)]
pub struct Cx {
    now: Millis,
    actions: Vec<Action>,
}

impl Cx {
    pub fn new(now: Millis) -> Cx {
        Cx {
            now,
            actions: Vec::new(),
        }
    }

    pub fn now(&self) -> Millis {
        self.now
    }

    pub fn take(self) -> Vec<Action> {
        self.actions
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn udp_open(&mut self, socket: Key, bind: Bind) {
        self.push(Action::UdpOpen { socket, bind });
    }

    pub fn udp_send(&mut self, socket: Key, to: SocketAddr, data: impl Into<Vec<u8>>) {
        self.push(Action::UdpSend {
            socket,
            to,
            data: data.into(),
        });
    }

    pub fn udp_close(&mut self, socket: Key) {
        self.push(Action::UdpClose { socket });
    }

    pub fn tcp_open(&mut self, socket: Key, to: SocketAddr) {
        self.push(Action::TcpOpen { socket, to });
    }

    /// Create a file on the host; see [`Action::FileOpen`].
    pub fn file_open(&mut self, file: Key, path: impl Into<String>) {
        self.push(Action::FileOpen {
            file,
            path: path.into(),
        });
    }

    /// Open an existing file for appending; see [`Action::FileAppend`].
    pub fn file_open_append(&mut self, file: Key, path: impl Into<String>) {
        self.push(Action::FileAppend {
            file,
            path: path.into(),
        });
    }

    pub fn file_write(&mut self, file: Key, data: impl Into<Vec<u8>>) {
        self.push(Action::FileWrite {
            file,
            data: data.into(),
        });
    }

    pub fn file_close(&mut self, file: Key) {
        self.push(Action::FileClose { file });
    }

    /// Read a whole file on the host; see [`Action::FileRead`].
    pub fn file_read(&mut self, file: Key, path: impl Into<String>, max_bytes: u64) {
        self.push(Action::FileRead {
            file,
            path: path.into(),
            max_bytes,
        });
    }

    /// Open a TCP stream through an SSH tunnel; see [`Action::TcpOpenSsh`].
    pub fn tcp_open_ssh(&mut self, socket: Key, tunnel: SshTunnel) {
        self.push(Action::TcpOpenSsh { socket, tunnel });
    }

    /// Open a TLS stream; see [`Action::TcpOpenTls`].
    pub fn tcp_open_tls(&mut self, socket: Key, target: TlsTarget) {
        self.push(Action::TcpOpenTls { socket, target });
    }

    pub fn tcp_send(&mut self, socket: Key, data: impl Into<Vec<u8>>) {
        self.push(Action::TcpSend {
            socket,
            data: data.into(),
        });
    }

    pub fn tcp_close(&mut self, socket: Key) {
        self.push(Action::TcpClose { socket });
    }

    pub fn tcp_listen(&mut self, socket: Key, port: u16) {
        self.push(Action::TcpListen { socket, port });
    }

    pub fn ws_open(&mut self, socket: Key, request: WsRequest) {
        self.push(Action::WsOpen { socket, request });
    }

    pub fn ws_send(&mut self, socket: Key, text: impl Into<String>) {
        self.push(Action::WsSend {
            socket,
            text: text.into(),
        });
    }

    pub fn ws_close(&mut self, socket: Key) {
        self.push(Action::WsClose { socket });
    }

    pub fn http(&mut self, id: RequestId, request: HttpRequest) {
        self.push(Action::Http { id, request });
    }

    pub fn sse_open(&mut self, stream: Key, request: HttpRequest) {
        self.push(Action::SseOpen { stream, request });
    }

    pub fn sse_close(&mut self, stream: Key) {
        self.push(Action::SseClose { stream });
    }

    pub fn set_timer(&mut self, key: Key, after: Millis) {
        self.push(Action::SetTimer { key, after });
    }

    pub fn cancel_timer(&mut self, key: Key) {
        self.push(Action::CancelTimer { key });
    }

    pub fn complete(&mut self, id: CommandId, result: CommandResult) {
        self.push(Action::Complete { id, result });
    }

    pub fn state(&mut self, patch: Value) {
        self.push(Action::State(patch));
    }

    pub fn connection(&mut self, connection: Connection) {
        self.push(Action::Connection(connection));
    }

    pub fn alive(&mut self) {
        self.push(Action::Alive);
    }

    /// A request was answered `millis` after it was sent. Report it for
    /// requests the device answers directly, not for pushed telemetry.
    pub fn round_trip(&mut self, millis: Millis) {
        self.push(Action::RoundTrip(millis));
    }

    pub fn log(&mut self, level: Level, message: impl Into<String>) {
        self.push(Action::Log {
            level,
            message: message.into(),
        });
    }

    /// Publish a frame on a stream; see [`Action::Frame`]. Publish only while
    /// [`Module::stream_watch`] has said the stream is watched.
    pub fn frame(&mut self, stream: Key, format: &'static str, data: impl Into<Vec<u8>>) {
        self.push(Action::Frame {
            stream,
            format,
            data: data.into(),
        });
    }
}

/// A device protocol implementation.
///
/// Commands reaching [`Module::command`] have already been checked against the
/// model's `supports` list and validated against the spec's parameters, with
/// defaults applied.
pub trait Module: Send + 'static {
    /// The session has started: open sockets, send subscriptions, arm timers.
    fn start(&mut self, cx: &mut Cx);

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params);

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        let _ = (cx, socket, from, data);
    }

    /// A UDP socket failed; it has been closed.
    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        let _ = (socket, message);
        cx.connection(Connection::Disconnected {
            reason: "socket error".into(),
        });
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        let _ = (cx, socket, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        let _ = (cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let _ = (cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        let _ = (cx, stream, input);
    }

    /// A file this module writes was closed, or failed.
    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        let _ = (cx, file, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key);

    /// A stream the spec declares gained its first watcher (`watching`) or
    /// lost its last. Produce frames with [`Cx::frame`] only in between, so a
    /// device is not asked for pictures nobody sees. Called again with the
    /// same value never; a module that reconnects keeps its own flag.
    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        let _ = (cx, stream, watching);
    }

    /// The session is closing: cancel subscriptions cleanly where the protocol
    /// allows it. Pending commands are failed with `Closed` by the session.
    fn stop(&mut self, cx: &mut Cx) {
        let _ = cx;
    }
}

/// What a module is constructed from when a device is opened.
#[derive(Debug, Clone)]
pub struct OpenContext {
    pub host: std::net::IpAddr,
    /// Overrides the protocol's default port.
    pub port: Option<u16>,
    pub model: String,
    pub channels: Option<u32>,
    pub settings: Params,
    /// `OpenRequest::monitor`: false means commands only, so the module
    /// sends no subscription, connect-time read or poll of its own.
    pub monitor: bool,
}
