//! TCP connections for device modules.
//!
//! One task per connection: it connects, reports the result, forwards every
//! received chunk and writes whatever the session hands it. Framing is the
//! module's job; this only moves bytes. A TLS stream is the same once its
//! handshake is done; a failed handshake closes it with a reason starting
//! `tls:`. A host file can be sent over a connection too, read on a thread
//! a few chunks ahead of the socket, so a file of any size streams at the
//! pace the peer takes it (an FTP data connection).

use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::module::{FileInput, Key, TcpInput, TlsTarget};
use crate::session::Inbound;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// TCP keepalive: idle time before the first probe, then the interval
/// between probes and how many go unanswered before the connection is
/// dropped. A peer that vanished without closing (a network change, a
/// suspended mobile app, a device switched off) is then noticed in about
/// half a minute even by a protocol that never replies, rather than after
/// the operating system's default of hours.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(10);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(not(windows))]
const KEEPALIVE_PROBES: u32 = 4;

pub(crate) struct Connection {
    pub(crate) generation: u64,
    pub(crate) writer: mpsc::UnboundedSender<Outgoing>,
    pub(crate) task: JoinHandle<()>,
}

/// What the session hands a connection to send.
pub(crate) enum Outgoing {
    Data(Vec<u8>),
    /// A host file, sent whole after what was handed over before it, then
    /// the sending side shut down; what is handed over after it is not
    /// sent. Reported on `file`: `Progress`, then `Closed` with the bytes
    /// sent, or `Failed`.
    File {
        file: Key,
        generation: u64,
        path: PathBuf,
    },
}

/// The chunk a host file is read and sent in.
const FILE_CHUNK: usize = 64 * 1024;
/// Chunks read ahead of the socket.
const FILE_AHEAD: usize = 4;
/// How often a file being sent reports its progress.
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    to: SocketAddr,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    start(socket, generation, inbound, Stream::Connect(to))
}

/// A TLS stream: connect, then the handshake, then as `spawn`.
pub(crate) fn spawn_tls(
    socket: Key,
    generation: u64,
    target: TlsTarget,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    start(socket, generation, inbound, Stream::Tls(target))
}

/// A connection the device opened to us, accepted by a listener.
pub(crate) fn adopt(
    socket: Key,
    generation: u64,
    stream: TcpStream,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    start(socket, generation, inbound, Stream::Accepted(stream))
}

enum Stream {
    Connect(SocketAddr),
    Tls(TlsTarget),
    Accepted(TcpStream),
}

async fn connect(to: SocketAddr) -> Result<TcpStream, String> {
    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(to)).await {
        Ok(Ok(s)) => {
            tune(&s);
            Ok(s)
        }
        Ok(Err(e)) => Err(format!("connect: {e}")),
        Err(_) => Err("connect: timed out".into()),
    }
}

/// No Nagle delay, and keepalive probes (see `KEEPALIVE_IDLE`).
fn tune(stream: &TcpStream) {
    let _ = stream.set_nodelay(true);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL);
    // Windows always sends ten probes.
    #[cfg(not(windows))]
    let keepalive = keepalive.with_retries(KEEPALIVE_PROBES);
    let _ = socket2::SockRef::from(stream).set_tcp_keepalive(&keepalive);
}

async fn handshake(
    target: &TlsTarget,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, String> {
    let name = crate::tls::server_name(&target.server_name)?;
    let tcp = connect(target.to).await?;
    let connector =
        tokio_rustls::TlsConnector::from(crate::tls::client_config(target.accept_invalid_certs));
    match tokio::time::timeout(HANDSHAKE_TIMEOUT, connector.connect(name, tcp)).await {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(crate::tls::describe(&e)),
        Err(_) => Err(format!("{} handshake timed out", crate::tls::PREFIX)),
    }
}

fn start(socket: Key, generation: u64, inbound: mpsc::Sender<Inbound>, how: Stream) -> Connection {
    let (writer, outgoing) = mpsc::unbounded_channel::<Outgoing>();
    let task = tokio::spawn(async move {
        let report = Report {
            socket,
            generation,
            inbound,
        };
        match how {
            Stream::Accepted(stream) => {
                tune(&stream);
                pump(stream, outgoing, &report).await;
            }
            Stream::Connect(to) => match connect(to).await {
                Ok(stream) => pump(stream, outgoing, &report).await,
                Err(reason) => {
                    report.send(TcpInput::Closed { reason }).await;
                }
            },
            Stream::Tls(target) => match handshake(&target).await {
                Ok(stream) => pump(stream, outgoing, &report).await,
                Err(reason) => {
                    report.send(TcpInput::Closed { reason }).await;
                }
            },
        }
    });
    Connection {
        generation,
        writer,
        task,
    }
}

/// Where a connection's events go.
pub(crate) struct Report {
    pub(crate) socket: Key,
    pub(crate) generation: u64,
    pub(crate) inbound: mpsc::Sender<Inbound>,
}

impl Report {
    async fn file(&self, file: Key, generation: u64, input: FileInput) -> bool {
        self.inbound
            .send(Inbound::File {
                file,
                generation,
                input,
            })
            .await
            .is_ok()
    }

    /// False once the session is gone.
    pub(crate) async fn send(&self, input: TcpInput) -> bool {
        self.inbound
            .send(Inbound::Tcp {
                socket: self.socket,
                generation: self.generation,
                input,
            })
            .await
            .is_ok()
    }
}

/// A host file being sent: its chunks arrive from a reader thread.
struct Sending {
    file: Key,
    generation: u64,
    chunks: mpsc::Receiver<Chunk>,
    bytes: u64,
    reported: Instant,
}

/// What the reader thread of a file being sent hands over.
enum Chunk {
    /// The file is open and this long; first.
    Length(u64),
    Data(Vec<u8>),
    Failed(String),
}

/// Read `path` on a thread, a few chunks ahead of whoever takes them. An
/// error ends the chunks; so does the receiver going away.
fn read_ahead(path: PathBuf) -> mpsc::Receiver<Chunk> {
    let (tx, rx) = mpsc::channel(FILE_AHEAD);
    std::thread::spawn(move || {
        let failed = |e: std::io::Error| Chunk::Failed(format!("{}: {e}", path.display()));
        let opened = std::fs::File::open(&path).and_then(|f| {
            let m = f.metadata()?;
            if m.is_file() {
                Ok((f, m.len()))
            } else {
                Err(std::io::Error::other("not a file"))
            }
        });
        let mut f = match opened {
            Ok((f, length)) => {
                if tx.blocking_send(Chunk::Length(length)).is_err() {
                    return;
                }
                f
            }
            Err(e) => {
                let _ = tx.blocking_send(failed(e));
                return;
            }
        };
        loop {
            let mut chunk = vec![0u8; FILE_CHUNK];
            match f.read(&mut chunk) {
                Ok(0) => return,
                Ok(n) => {
                    chunk.truncate(n);
                    if tx.blocking_send(Chunk::Data(chunk)).is_err() {
                        return;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    let _ = tx.blocking_send(failed(e));
                    return;
                }
            }
        }
    });
    rx
}

async fn write_out<W: AsyncWrite + Unpin>(write: &mut W, data: &[u8]) -> std::io::Result<()> {
    write.write_all(data).await?;
    // TLS buffers records until flushed.
    write.flush().await
}

/// Report the stream connected, then move bytes both ways until either side
/// ends it.
pub(crate) async fn pump<S>(
    stream: S,
    mut outgoing: mpsc::UnboundedReceiver<Outgoing>,
    report: &Report,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !report.send(TcpInput::Connected).await {
        return;
    }
    let (mut read, mut write) = tokio::io::split(stream);
    let mut buf = vec![0u8; 16 * 1024];
    let mut sending: Option<Sending> = None;
    // Once a file has been sent the sending side is shut down.
    let mut shut = false;
    let reason = loop {
        tokio::select! {
            result = read.read(&mut buf) => match result {
                Ok(0) => break "closed by the device".to_string(),
                Ok(n) => {
                    if !report.send(TcpInput::Data(buf[..n].to_vec())).await {
                        return;
                    }
                }
                Err(e) => break e.to_string(),
            },
            chunk = next_chunk(&mut sending), if sending.is_some() => {
                let Some(s) = sending.as_mut() else { continue };
                match chunk {
                    Some(Chunk::Length(bytes)) => {
                        if !report.file(s.file, s.generation, FileInput::Opened { bytes }).await {
                            return;
                        }
                    }
                    Some(Chunk::Data(data)) => {
                        if let Err(e) = write_out(&mut write, &data).await {
                            let message = e.to_string();
                            let input = FileInput::Failed { message: message.clone() };
                            report.file(s.file, s.generation, input).await;
                            break message;
                        }
                        s.bytes += data.len() as u64;
                        if s.reported.elapsed() >= PROGRESS_EVERY {
                            s.reported = Instant::now();
                            let input = FileInput::Progress { bytes: s.bytes };
                            if !report.file(s.file, s.generation, input).await {
                                return;
                            }
                        }
                    }
                    Some(Chunk::Failed(message)) => {
                        // A file cut short must not look sent whole.
                        let input = FileInput::Failed { message: message.clone() };
                        report.file(s.file, s.generation, input).await;
                        break format!("host file: {message}");
                    }
                    None => {
                        let (file, generation, bytes) = (s.file, s.generation, s.bytes);
                        sending = None;
                        let _ = write.shutdown().await;
                        shut = true;
                        if !report.file(file, generation, FileInput::Closed { bytes }).await {
                            return;
                        }
                    }
                }
            },
            data = outgoing.recv(), if sending.is_none() => match data {
                Some(Outgoing::Data(_)) if shut => {}
                Some(Outgoing::Data(data)) => {
                    if let Err(e) = write_out(&mut write, &data).await {
                        break e.to_string();
                    }
                }
                Some(Outgoing::File { file, generation, .. }) if shut => {
                    let message = "the connection's sending side is already shut down".to_string();
                    if !report.file(file, generation, FileInput::Failed { message }).await {
                        return;
                    }
                }
                Some(Outgoing::File { file, generation, path }) => {
                    sending = Some(Sending {
                        file,
                        generation,
                        chunks: read_ahead(path),
                        bytes: 0,
                        reported: Instant::now(),
                    });
                }
                None => {
                    let _ = write.shutdown().await;
                    return;
                }
            },
        }
    };
    report.send(TcpInput::Closed { reason }).await;
}

/// The next chunk of the file being sent; never ready without one.
async fn next_chunk(sending: &mut Option<Sending>) -> Option<Chunk> {
    match sending {
        Some(s) => s.chunks.recv().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// A TLS server with a self-signed certificate that echoes what it gets.
    async fn tls_echo() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(crate::tls::testing::server_config());
        tokio::spawn(async move {
            loop {
                let (tcp, _) = listener.accept().await.unwrap();
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let Ok(stream) = acceptor.accept(tcp).await else {
                        return;
                    };
                    let (mut r, mut w) = tokio::io::split(stream);
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = r.read(&mut buf).await {
                        if n == 0 || w.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                        let _ = w.flush().await;
                    }
                });
            }
        });
        addr
    }

    async fn next(rx: &mut mpsc::Receiver<Inbound>) -> TcpInput {
        match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(Inbound::Tcp { input, .. })) => input,
            _ => panic!("expected a TCP event"),
        }
    }

    fn target(to: SocketAddr, accept_invalid_certs: bool) -> TlsTarget {
        TlsTarget {
            to,
            server_name: "127.0.0.1".into(),
            accept_invalid_certs,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_self_signed_device_is_reached_when_invalid_certs_are_accepted() {
        let addr = tls_echo().await;
        let (tx, mut rx) = mpsc::channel(16);
        let c = spawn_tls("a", 1, target(addr, true), tx);
        assert_eq!(next(&mut rx).await, TcpInput::Connected);
        c.writer.send(Outgoing::Data(b"hello".to_vec())).unwrap();
        assert_eq!(next(&mut rx).await, TcpInput::Data(b"hello".to_vec()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_self_signed_certificate_is_refused_otherwise() {
        let addr = tls_echo().await;
        let (tx, mut rx) = mpsc::channel(16);
        let _c = spawn_tls("a", 1, target(addr, false), tx);
        match next(&mut rx).await {
            TcpInput::Closed { reason } => {
                assert!(reason.starts_with("tls:"), "{reason}");
                assert!(reason.contains("accept_invalid_certs"), "{reason}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_device_that_does_not_speak_tls_fails_the_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.write_all(b"plain text, not a TLS record\r\n").await;
        });
        let (tx, mut rx) = mpsc::channel(16);
        let _c = spawn_tls("a", 1, target(addr, true), tx);
        match next(&mut rx).await {
            TcpInput::Closed { reason } => assert!(reason.starts_with("tls:"), "{reason}"),
            other => panic!("expected a close, got {other:?}"),
        }
    }

    /// A host file goes out whole after what was sent before it, then the
    /// sending side is shut down; a missing file fails without sending.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_host_file_is_sent_then_the_sending_side_shut() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let received = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut got = Vec::new();
            s.read_to_end(&mut got).await.unwrap();
            got
        });
        let dir = std::env::temp_dir().join(format!("meros-tcp-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip.bin");
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
        std::fs::write(&path, &body).unwrap();

        let (tx, mut rx) = mpsc::channel(64);
        let c = spawn("data", 1, addr, tx);
        c.writer.send(Outgoing::Data(b"head".to_vec())).unwrap();
        c.writer
            .send(Outgoing::File {
                file: "f",
                generation: 7,
                path: path.clone(),
            })
            .unwrap();
        let mut file_inputs = Vec::new();
        loop {
            match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
                Ok(Some(Inbound::File {
                    file: "f",
                    generation: 7,
                    input,
                })) => {
                    let end = matches!(input, FileInput::Closed { .. } | FileInput::Failed { .. });
                    file_inputs.push(input);
                    if end {
                        break;
                    }
                }
                Ok(Some(_)) => {}
                _ => panic!("expected the file to be sent"),
            }
        }
        assert_eq!(
            file_inputs.first(),
            Some(&FileInput::Opened { bytes: 300_000 })
        );
        assert_eq!(
            file_inputs.last(),
            Some(&FileInput::Closed { bytes: 300_000 })
        );
        let mut expected = b"head".to_vec();
        expected.extend_from_slice(&body);
        assert_eq!(received.await.unwrap(), expected);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _keep = listener.accept().await;
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let (tx, mut rx) = mpsc::channel(64);
        let c = spawn("data", 2, addr, tx);
        c.writer
            .send(Outgoing::File {
                file: "f",
                generation: 8,
                path: dir.join("missing.bin"),
            })
            .unwrap();
        loop {
            match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
                Ok(Some(Inbound::File { input, .. })) => {
                    assert!(matches!(input, FileInput::Failed { .. }), "{input:?}");
                    break;
                }
                Ok(Some(_)) => {}
                _ => panic!("expected the file to fail"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A peer that vanishes without closing is noticed by keepalive probes,
    /// including on connections the device opened to us.
    #[tokio::test(flavor = "multi_thread")]
    async fn connections_probe_a_silent_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = tokio::spawn(async move { listener.accept().await.unwrap().0 });
        let stream = connect(addr).await.unwrap();
        let sock = socket2::SockRef::from(&stream);
        assert!(sock.keepalive().unwrap());
        assert!(sock.nodelay().unwrap());
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios"
        ))]
        {
            assert_eq!(sock.keepalive_time().unwrap(), KEEPALIVE_IDLE);
            assert_eq!(sock.keepalive_interval().unwrap(), KEEPALIVE_INTERVAL);
            assert_eq!(sock.keepalive_retries().unwrap(), KEEPALIVE_PROBES);
        }
        let peer = accepted.await.unwrap();
        tune(&peer);
        assert!(socket2::SockRef::from(&peer).keepalive().unwrap());
    }
}
