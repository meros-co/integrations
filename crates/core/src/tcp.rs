//! TCP connections for device modules.
//!
//! One task per connection: it connects, reports the result, forwards every
//! received chunk and writes whatever the session hands it. Framing is the
//! module's job; this only moves bytes. A TLS stream is the same once its
//! handshake is done; a failed handshake closes it with a reason starting
//! `tls:`.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::module::{Key, TcpInput, TlsTarget};
use crate::session::Inbound;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Connection {
    pub(crate) generation: u64,
    pub(crate) writer: mpsc::UnboundedSender<Vec<u8>>,
    pub(crate) task: JoinHandle<()>,
}

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
            let _ = s.set_nodelay(true);
            Ok(s)
        }
        Ok(Err(e)) => Err(format!("connect: {e}")),
        Err(_) => Err("connect: timed out".into()),
    }
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
    let (writer, outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let task = tokio::spawn(async move {
        let report = Report {
            socket,
            generation,
            inbound,
        };
        match how {
            Stream::Accepted(stream) => {
                let _ = stream.set_nodelay(true);
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

/// Report the stream connected, then move bytes both ways until either side
/// ends it.
pub(crate) async fn pump<S>(
    stream: S,
    mut outgoing: mpsc::UnboundedReceiver<Vec<u8>>,
    report: &Report,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !report.send(TcpInput::Connected).await {
        return;
    }
    let (mut read, mut write) = tokio::io::split(stream);
    let mut buf = vec![0u8; 16 * 1024];
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
            data = outgoing.recv() => match data {
                Some(data) => {
                    let written = match write.write_all(&data).await {
                        // TLS buffers records until flushed.
                        Ok(()) => write.flush().await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = written {
                        break e.to_string();
                    }
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
        c.writer.send(b"hello".to_vec()).unwrap();
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
}
