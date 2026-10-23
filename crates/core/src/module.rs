//! The interface every device module implements.
//!
//! A module is protocol logic only. It never opens a socket or reads a clock:
//! it is handed commands, inbound data and timer expiries, and answers with
//! [`Action`]s that the session carries out. That keeps a module deterministic,
//! so it can be tested by feeding it recorded bytes and synthetic time, and it
//! means no module can differ from another in how it uses the network.

use std::net::SocketAddr;
use std::path::PathBuf;

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
    /// A body sent in pieces in place of `body`, with `Content-Length` their
    /// total: each file among them is read from disk as it is sent, never
    /// held whole (a binary parameter given as a host file, SPEC.md §4).
    pub body_stream: Option<Vec<BodyPiece>>,
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

/// One piece of a streamed request body.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyPiece {
    Bytes(Vec<u8>),
    /// The first `len` bytes of the host file at `path`, read as they are
    /// sent. A file shorter than that when it is sent fails the request.
    File {
        path: PathBuf,
        len: u64,
    },
}

impl BodyPiece {
    /// The piece's length in bytes.
    pub fn len(&self) -> u64 {
        match self {
            BodyPiece::Bytes(bytes) => bytes.len() as u64,
            BodyPiece::File { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl HttpRequest {
    /// The whole body in memory, its files read in full: for checking what
    /// a request sends (conformance vectors, tests), never for sending it.
    pub fn body_bytes(&self) -> std::io::Result<Option<Vec<u8>>> {
        let Some(pieces) = &self.body_stream else {
            return Ok(self.body.clone());
        };
        let mut out = Vec::new();
        for piece in pieces {
            match piece {
                BodyPiece::Bytes(bytes) => out.extend_from_slice(bytes),
                BodyPiece::File { path, len } => {
                    use std::io::Read;
                    let start = out.len();
                    std::fs::File::open(path)?
                        .take(*len)
                        .read_to_end(&mut out)?;
                    if ((out.len() - start) as u64) < *len {
                        return Err(std::io::ErrorKind::UnexpectedEof.into());
                    }
                }
            }
        }
        Ok(Some(out))
    }
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
    /// Plain text the device sends on connecting, before the handshake
    /// starts (Samsung MDC's secured mode greets with `MDCSTART<<TLS>>`). It
    /// is read and checked first; anything else fails the handshake with a
    /// reason starting `tls:`. `None` starts the handshake at once.
    pub preamble: Option<Vec<u8>>,
}

/// A serial port a module asks the session to open: RS-232, RS-422 or a USB
/// serial adapter on the host running the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPort {
    /// The port's name: `COM3` on Windows, a device path such as
    /// `/dev/ttyUSB0` (Linux) or `/dev/cu.usbserial-1420` (macOS) elsewhere.
    pub path: String,
    /// Bits per second.
    pub baud_rate: u32,
    /// Bits per character, 5 to 8.
    pub data_bits: u8,
    pub parity: Parity,
    /// 1 or 2.
    pub stop_bits: u8,
    pub flow_control: FlowControl,
}

/// A serial port's parity bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
}

/// How a serial port paces what is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    None,
    /// Software flow control: the XON (11H) and XOFF (13H) characters.
    XonXoff,
    /// Hardware flow control on the RTS and CTS lines.
    RtsCts,
}

/// The serial settings every spec on a serial port declares, by name
/// (SPEC.md §2, `line-serial`): the port, then its line settings.
pub const SERIAL_SETTINGS: [&str; 6] = [
    "serial_port",
    "baud_rate",
    "data_bits",
    "parity",
    "stop_bits",
    "flow_control",
];

impl SerialPort {
    /// The port and line settings from a device's settings, by the names in
    /// [`SERIAL_SETTINGS`]. A line setting left out is 9600 bit/s, 8 data
    /// bits, no parity, 1 stop bit and no flow control; `baud_rate` may be an
    /// integer or an enum of digits. The port has no default.
    pub fn from_settings(settings: &crate::catalog::Params) -> Result<SerialPort, String> {
        use serde_json::Value;
        let path = settings
            .get("serial_port")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or("no serial port is set: give the serial_port setting")?
            .to_string();
        let int = |name: &str, default: u64| -> Result<u64, String> {
            match settings.get(name) {
                None | Some(Value::Null) => Ok(default),
                Some(Value::String(s)) => s
                    .trim()
                    .parse()
                    .map_err(|_| format!("{name} '{s}' is not a number")),
                Some(v) => v.as_u64().ok_or(format!("{name} {v} is not a number")),
            }
        };
        let baud_rate = int("baud_rate", 9600)?;
        if !(1..=u32::MAX as u64).contains(&baud_rate) {
            return Err(format!("baud_rate {baud_rate} is out of range"));
        }
        let data_bits = int("data_bits", 8)?;
        if !(5..=8).contains(&data_bits) {
            return Err(format!("data_bits {data_bits} is not 5 to 8"));
        }
        let stop_bits = int("stop_bits", 1)?;
        if !(1..=2).contains(&stop_bits) {
            return Err(format!("stop_bits {stop_bits} is not 1 or 2"));
        }
        let text = |name: &str, default: &'static str| -> String {
            settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or(default)
                .to_string()
        };
        let parity = match text("parity", "none").as_str() {
            "none" => Parity::None,
            "odd" => Parity::Odd,
            "even" => Parity::Even,
            other => return Err(format!("parity '{other}' is not none, odd or even")),
        };
        let flow_control = match text("flow_control", "none").as_str() {
            "none" => FlowControl::None,
            "xon-xoff" => FlowControl::XonXoff,
            "rts-cts" => FlowControl::RtsCts,
            other => {
                return Err(format!(
                    "flow_control '{other}' is not none, xon-xoff or rts-cts"
                ))
            }
        };
        Ok(SerialPort {
            path,
            baud_rate: baud_rate as u32,
            data_bits: data_bits as u8,
            parity,
            stop_bits: stop_bits as u8,
            flow_control,
        })
    }
}

/// Where a native module that offers both reaches its device: a TCP
/// connection, or a serial port of the host. Both are reported through
/// [`Module::tcp`], so the module's protocol code is the same for either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Tcp(std::net::SocketAddr),
    Serial(SerialPort),
}

impl Link {
    /// The link a device's `link` setting chooses: `tcp` (the default) to
    /// the device's host and port, `serial_port` and its line settings for
    /// `serial`. A TCP link needs a host: a device opened without one (as a
    /// device on a serial port may be) is refused here.
    pub fn from_settings(ctx: &OpenContext, default_port: u16) -> Result<Link, String> {
        match ctx.settings.get("link").and_then(serde_json::Value::as_str) {
            None | Some("tcp") => {
                if ctx.host.is_unspecified() {
                    return Err("a TCP link needs the device's host".into());
                }
                Ok(Link::Tcp(std::net::SocketAddr::new(
                    ctx.host,
                    ctx.port.unwrap_or(default_port),
                )))
            }
            Some("serial") => Ok(Link::Serial(SerialPort::from_settings(&ctx.settings)?)),
            Some(other) => Err(format!("link '{other}' is not tcp or serial")),
        }
    }

    /// Open it under `socket`, replacing what was open there.
    pub fn open(&self, cx: &mut Cx, socket: Key) {
        match self {
            Link::Tcp(to) => cx.tcp_open(socket, *to),
            Link::Serial(port) => cx.serial_open(socket, port.clone()),
        }
    }
}

/// A username and password.
#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HttpResponse {
    pub status: u16,
    /// The response's headers in the order received, names lowercased; a
    /// header sent more than once (`Set-Cookie`) appears once per value.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Every value of a header, by name in any case.
    pub fn header_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.headers
            .iter()
            .filter(move |(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
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
    /// before anything is appended. For a file sent with `tcp_send_file`,
    /// it is open for reading and `bytes` is its length.
    Opened { bytes: u64 },
    /// Every chunk was written and the file closed. `bytes` is the file's
    /// length: what was written, plus, when appending, what was there before.
    Closed { bytes: u64 },
    /// The file could not be created, written or read. Reported once; later
    /// writes are dropped.
    Failed { message: String },
    /// A file asked for with `file_read`, whole.
    Read { data: Vec<u8> },
    /// A file sent with `tcp_send_file`: `bytes` have gone out so far.
    /// Reported every 200 ms or so while it is sent.
    Progress { bytes: u64 },
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
    /// A ping, or a pong to a ping the module did not send, arrived: proof
    /// of life, nothing to handle.
    Activity,
    /// A pong arrived, with its payload: the answer to a ping the module
    /// sent with `ws_ping` (RFC 6455 §5.5.3), or one the peer sent unasked.
    Pong(Vec<u8>),
    /// The subprotocol the peer chose in its handshake answer, reported
    /// before `Opened` when it chose one (RFC 6455 §4.1).
    Subprotocol(String),
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
    /// A fixed port owned by this session alone, hearing every sender rather
    /// than only the device's address: for a listener that serves whoever
    /// sends (OSC from any control surface). `address` is the local address
    /// to bind, the core's bind address when absent.
    Any {
        address: Option<std::net::IpAddr>,
        port: u16,
    },
}

/// Work too heavy for the session's own task, such as decoding or encoding
/// a large picture: the session runs it on a blocking thread and reports
/// its result to [`Module::computed`], so the device keeps being served
/// meanwhile. The work is a pure function of what it captured; it touches
/// no socket, file or clock.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct Task(
    std::sync::Arc<std::sync::Mutex<Option<Box<dyn FnOnce() -> Result<Vec<u8>, String> + Send>>>>,
);

impl Task {
    pub fn new(work: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static) -> Task {
        Task(std::sync::Arc::new(std::sync::Mutex::new(Some(Box::new(
            work,
        )))))
    }

    /// Run the work; a task runs once, and a second run is an error.
    pub fn run(&self) -> Result<Vec<u8>, String> {
        let work = self.0.lock().map(|mut w| w.take()).unwrap_or(None);
        match work {
            Some(work) => work(),
            None => Err("the task has already run".into()),
        }
    }
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Task")
    }
}

impl PartialEq for Task {
    fn eq(&self, other: &Task) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
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
    /// Open a serial port, replacing any connection open under this key.
    /// Reported like `TcpOpen`: `Connected` once the port is open, `Data`
    /// as bytes arrive, and `Closed` when it cannot be opened or fails (a
    /// USB adapter unplugged); `TcpSend`, `TcpSendFile` and `TcpClose` work
    /// on it as on a connection. A port that cannot be opened closes with a
    /// reason starting `serial:`.
    SerialOpen {
        socket: Key,
        port: SerialPort,
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
    /// file. No size is refused here: what the file may hold is the
    /// device's to judge.
    FileRead {
        file: Key,
        path: String,
    },
    TcpClose {
        socket: Key,
    },
    /// Send a host file over an open connection, after what was sent on it
    /// before, then shut down the connection's sending side (the end of an
    /// FTP upload); nothing sent after it goes out. The file is read a few
    /// chunks ahead of the socket, so it streams at the peer's pace whatever
    /// its size, while what the peer sends keeps arriving. Reported on
    /// `file` as `FileInput::Opened` with the file's length, `Progress`
    /// while it goes out, then `Closed` with the bytes sent,
    /// or `Failed` (the file cannot be read, the connection is not open or
    /// fails). Closing the connection stops it, unreported.
    TcpSendFile {
        socket: Key,
        file: Key,
        path: String,
    },
    /// Run a [`Task`] off the session's task; its result goes to
    /// [`Module::computed`] under `key`. A newer task under the same key
    /// replaces an older one still running, whose result is then dropped.
    Compute {
        key: Key,
        task: Task,
    },
    /// Accept connections from the device on a local TCP port, shared with
    /// other sessions and routed by peer address. Each accepted connection is
    /// reported on `socket` as `TcpInput::Connected`, replacing any before it.
    TcpListen {
        socket: Key,
        port: u16,
    },
    /// Listen on a local TCP port owned by this session alone, accepting any
    /// number of connections from any peer at once (up to a limit the session
    /// sets). Each is reported through [`Module::tcp_client`] with its peer's
    /// address. A port that cannot be bound is reported to
    /// [`Module::socket_error`]. `TcpClose` on the key stops listening and
    /// closes every connection. `address` is the local address to bind, the
    /// core's bind address when absent.
    TcpServe {
        socket: Key,
        address: Option<std::net::IpAddr>,
        port: u16,
    },
    /// Write to one connection accepted by `TcpServe`; nothing if it is gone.
    TcpClientSend {
        socket: Key,
        peer: SocketAddr,
        data: Vec<u8>,
    },
    /// Shut down the sending side of one connection accepted by `TcpServe`
    /// once what was sent to it before is written, so the peer reads to the
    /// end (an HTTP response marked `Connection: close`). The connection is
    /// reported `Closed` when the peer closes it.
    TcpClientShutdown {
        socket: Key,
        peer: SocketAddr,
    },
    /// Close one connection accepted by `TcpServe`. Not reported back: the
    /// module knows.
    TcpClientClose {
        socket: Key,
        peer: SocketAddr,
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
    /// Send a binary message, for protocols that frame bytes in WebSocket
    /// binary messages (AES70's OCP.1).
    WsSendBinary {
        socket: Key,
        data: Vec<u8>,
    },
    /// Send a ping with this payload, at most 125 bytes (RFC 6455 §5.5); its
    /// pong arrives as `WsInput::Pong`.
    WsPing {
        socket: Key,
        data: Vec<u8>,
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
    /// Report one received message as an `Event::Message`, every time, even
    /// when it repeats the last: for what state cannot carry, such as a
    /// button pressed twice.
    Message {
        address: String,
        types: Option<String>,
        args: Value,
        source: SocketAddr,
    },
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
    /// New values of the device's own settings the module obtained itself,
    /// such as OAuth tokens it refreshed: reported as an `Event::Credentials`
    /// for the consumer to persist, and kept as the device's settings so a
    /// later `update_settings` merges onto them.
    Credentials(Params),
}

/// Collects a module's actions for one callback.
#[derive(Debug)]
pub struct Cx {
    now: Millis,
    /// Wall-clock time, milliseconds since the Unix epoch.
    unix: Millis,
    actions: Vec<Action>,
}

impl Cx {
    /// A context at `now`, whose wall clock reads the same: synthetic time
    /// for tests, where the Unix epoch is the session's start.
    pub fn new(now: Millis) -> Cx {
        Cx::at(now, now)
    }

    /// A context at `now` on the session's clock and `unix` milliseconds
    /// since the Unix epoch on the wall clock.
    pub fn at(now: Millis, unix: Millis) -> Cx {
        Cx {
            now,
            unix,
            actions: Vec::new(),
        }
    }

    pub fn now(&self) -> Millis {
        self.now
    }

    /// Wall-clock time in milliseconds since the Unix epoch, for what a
    /// service states as a date (a token's expiry). Use [`Cx::now`] for
    /// timeouts and intervals.
    pub fn unix_millis(&self) -> Millis {
        self.unix
    }

    /// Report settings the module obtained itself; see [`Action::Credentials`].
    pub fn credentials(&mut self, settings: Params) {
        self.push(Action::Credentials(settings));
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
    pub fn file_read(&mut self, file: Key, path: impl Into<String>) {
        self.push(Action::FileRead {
            file,
            path: path.into(),
        });
    }

    /// Run work off the session's task; see [`Action::Compute`].
    pub fn compute(
        &mut self,
        key: Key,
        work: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static,
    ) {
        self.push(Action::Compute {
            key,
            task: Task::new(work),
        });
    }

    /// Open a serial port; see [`Action::SerialOpen`].
    pub fn serial_open(&mut self, socket: Key, port: SerialPort) {
        self.push(Action::SerialOpen { socket, port });
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

    /// Send a host file over a connection; see [`Action::TcpSendFile`].
    pub fn tcp_send_file(&mut self, socket: Key, file: Key, path: impl Into<String>) {
        self.push(Action::TcpSendFile {
            socket,
            file,
            path: path.into(),
        });
    }

    pub fn tcp_listen(&mut self, socket: Key, port: u16) {
        self.push(Action::TcpListen { socket, port });
    }

    /// Accept connections from any peer; see [`Action::TcpServe`].
    pub fn tcp_serve(&mut self, socket: Key, address: Option<std::net::IpAddr>, port: u16) {
        self.push(Action::TcpServe {
            socket,
            address,
            port,
        });
    }

    pub fn tcp_client_send(&mut self, socket: Key, peer: SocketAddr, data: impl Into<Vec<u8>>) {
        self.push(Action::TcpClientSend {
            socket,
            peer,
            data: data.into(),
        });
    }

    pub fn tcp_client_shutdown(&mut self, socket: Key, peer: SocketAddr) {
        self.push(Action::TcpClientShutdown { socket, peer });
    }

    pub fn tcp_client_close(&mut self, socket: Key, peer: SocketAddr) {
        self.push(Action::TcpClientClose { socket, peer });
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

    pub fn ws_send_binary(&mut self, socket: Key, data: impl Into<Vec<u8>>) {
        self.push(Action::WsSendBinary {
            socket,
            data: data.into(),
        });
    }

    pub fn ws_ping(&mut self, socket: Key, data: impl Into<Vec<u8>>) {
        self.push(Action::WsPing {
            socket,
            data: data.into(),
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

    /// Report one received message; see [`Action::Message`].
    pub fn message(
        &mut self,
        address: impl Into<String>,
        types: Option<String>,
        args: Value,
        source: SocketAddr,
    ) {
        self.push(Action::Message {
            address: address.into(),
            types,
            args,
            source,
        });
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

    /// Something happened on one connection accepted by `TcpServe`, told
    /// apart from the others by its peer's address.
    fn tcp_client(&mut self, cx: &mut Cx, socket: Key, peer: SocketAddr, input: TcpInput) {
        let _ = (cx, socket, peer, input);
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

    /// A task started with [`Cx::compute`] finished.
    fn computed(&mut self, cx: &mut Cx, key: Key, result: Result<Vec<u8>, String>) {
        let _ = (cx, key, result);
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

    /// The consumer changed the device's settings (`Core::update_settings`).
    /// `settings` is the whole set, validated against the spec, with the
    /// change merged in. A module that can take them while running applies
    /// them and answers [`SettingsUpdate::Applied`]. The default answers
    /// [`SettingsUpdate::Restart`]: the session stops this module as on
    /// closing, without a `closed` event, fails its pending commands, and
    /// starts one built afresh from the new settings under the same device
    /// id, keeping the device's state and stream watchers.
    fn update_settings(&mut self, cx: &mut Cx, settings: &Params) -> SettingsUpdate {
        let _ = (cx, settings);
        SettingsUpdate::Restart
    }

    /// The consumer asked for the device to reconnect now
    /// (`Core::reconnect`): its network changed, or it resumed after being
    /// suspended, so a connection may be dead without either end knowing. A
    /// module holding something the consumer asked for that the device
    /// forgets with the connection (a tally, a level subscription) drops its
    /// connections, opens them again at once and asks for it again, then
    /// answers true. The default answers false: the session restarts the
    /// module from the device's settings, as for a settings change.
    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        let _ = cx;
        false
    }
}

/// What a module did with new settings.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsUpdate {
    /// Applied while running; commands in flight carry on.
    Applied,
    /// Rebuild the module from the new settings.
    Restart,
    /// The settings cannot be used; nothing changed.
    Rejected(String),
}

/// What a module is constructed from when a device is opened.
#[derive(Debug, Clone)]
pub struct OpenContext {
    pub host: std::net::IpAddr,
    /// The host name the device was opened with, when it was a name rather
    /// than an address. HTTP, websocket and event-stream URLs carry it, so
    /// TLS checks the certificate against it and `Host` names it: a cloud
    /// API (`api.planningcenteronline.com`) serves neither by address.
    pub host_name: Option<String>,
    /// Overrides the protocol's default port.
    pub port: Option<u16>,
    pub model: String,
    pub channels: Option<u32>,
    pub settings: Params,
    /// `OpenRequest::monitor`: false means commands only, so the module
    /// sends no subscription, connect-time read or poll of its own.
    pub monitor: bool,
}
