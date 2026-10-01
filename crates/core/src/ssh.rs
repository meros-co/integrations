//! TCP streams through an SSH tunnel, for devices that accept their control
//! protocol only over SSH port forwarding (Sony cameras with SSH on).
//!
//! Each stream is a `direct-tcpip` channel. Streams to the same device with the
//! same login share one SSH session, so a protocol needing two connections
//! (PTP-IP's command and event connections) logs in once. To the module a
//! tunnelled stream looks like any TCP stream: `Connected`, `Data`, `Closed`.
//!
//! A host key that does not match the configured fingerprint, or a login the
//! device refuses, closes the stream with a reason starting `ssh refused:`.
//! Modules treat that as a terminal credential refusal: repeated wrong logins
//! lock some devices out.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, AuthResult, Handle};
use russh::keys::{HashAlg, PublicKeyOrCertificate};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::module::{Key, SshTunnel, TcpInput};
use crate::session::Inbound;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(15);

/// The prefix of a `Closed` reason that is a refusal, not a fault.
pub const REFUSED: &str = "ssh refused:";

/// A device's SSH server and the username logged in with.
type Login = (SocketAddr, String);

/// One SSH session per device address and login, shared by its streams.
#[derive(Clone, Default)]
pub(crate) struct Sessions {
    open: Arc<tokio::sync::Mutex<HashMap<Login, Arc<Handle<Client>>>>>,
}

pub(crate) struct Client {
    expected: Option<String>,
    /// The fingerprint the device presented, for the refusal message.
    seen: Arc<Mutex<Option<String>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            // Certificates are not configured, so not trusted.
            return Ok(false);
        };
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        *self.seen.lock().unwrap() = Some(fingerprint.clone());
        Ok(match &self.expected {
            Some(expected) => same_fingerprint(expected, &fingerprint),
            // No fingerprint configured: the host is not verified.
            None => true,
        })
    }
}

/// Fingerprints as OpenSSH prints them ("SHA256:base64"), with or without the
/// prefix and trailing padding.
fn same_fingerprint(configured: &str, presented: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_start_matches("SHA256:")
            .trim_end_matches('=')
            .to_string()
    };
    norm(configured) == norm(presented)
}

/// Why a stream could not be opened.
enum Failure {
    Refused(String),
    Fault(String),
}

impl Sessions {
    async fn session(&self, tunnel: &SshTunnel) -> Result<Arc<Handle<Client>>, Failure> {
        let mut open = self.open.lock().await;
        let key = (tunnel.ssh, tunnel.username.clone());
        if let Some(handle) = open.get(&key) {
            if !handle.is_closed() {
                return Ok(handle.clone());
            }
        }
        let handle = Arc::new(connect(tunnel).await?);
        open.insert(key, handle.clone());
        Ok(handle)
    }
}

async fn connect(tunnel: &SshTunnel) -> Result<Handle<Client>, Failure> {
    let mut config = client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(10)),
        keepalive_max: 3,
        ..Default::default()
    };
    if let Some(cipher) = &tunnel.cipher {
        // The device's cipher first; the defaults after it.
        let wanted = russh::cipher::Name::try_from(cipher.as_str())
            .map_err(|_| Failure::Fault(format!("ssh: unknown cipher {cipher}")))?;
        let mut ciphers = vec![wanted];
        ciphers.extend(config.preferred.cipher.iter().filter(|c| **c != wanted));
        config.preferred.cipher = ciphers.into();
    }
    let seen = Arc::new(Mutex::new(None));
    let handler = Client {
        expected: tunnel.fingerprint.clone(),
        seen: seen.clone(),
    };
    let connecting = client::connect(Arc::new(config), tunnel.ssh, handler);
    let mut handle = match tokio::time::timeout(CONNECT_TIMEOUT + LOGIN_TIMEOUT, connecting).await {
        Ok(Ok(handle)) => handle,
        Ok(Err(russh::Error::UnknownKey)) => {
            let presented = seen.lock().unwrap().clone().unwrap_or_default();
            return Err(Failure::Refused(format!(
                "{REFUSED} the device's host key {presented} does not match the configured fingerprint"
            )));
        }
        Ok(Err(e)) => return Err(Failure::Fault(format!("ssh: {e}"))),
        Err(_) => return Err(Failure::Fault("ssh: connect timed out".into())),
    };
    let login = handle.authenticate_password(&tunnel.username, &tunnel.password);
    match tokio::time::timeout(LOGIN_TIMEOUT, login).await {
        Ok(Ok(AuthResult::Success)) => Ok(handle),
        Ok(Ok(AuthResult::Failure { .. })) => Err(Failure::Refused(format!(
            "{REFUSED} the device refused the login for {}",
            tunnel.username
        ))),
        Ok(Err(e)) => Err(Failure::Fault(format!("ssh: {e}"))),
        Err(_) => Err(Failure::Fault("ssh: login timed out".into())),
    }
}

/// Open a stream to `tunnel.target_host:target_port` as the device sees it,
/// reporting it to the session like a TCP stream.
pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    tunnel: SshTunnel,
    sessions: Sessions,
    inbound: mpsc::Sender<Inbound>,
) -> crate::tcp::Connection {
    let (writer, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let task = tokio::spawn(async move {
        let send = |input: TcpInput| {
            let inbound = inbound.clone();
            async move {
                inbound
                    .send(Inbound::Tcp {
                        socket,
                        generation,
                        input,
                    })
                    .await
                    .is_ok()
            }
        };
        let closed = |reason: String| send(TcpInput::Closed { reason });

        let handle = match sessions.session(&tunnel).await {
            Ok(handle) => handle,
            Err(Failure::Refused(reason) | Failure::Fault(reason)) => {
                closed(reason).await;
                return;
            }
        };
        let channel = match handle
            .channel_open_direct_tcpip(
                tunnel.target_host.clone(),
                u32::from(tunnel.target_port),
                "127.0.0.1",
                0,
            )
            .await
        {
            Ok(channel) => channel,
            Err(e) => {
                closed(format!("ssh: forwarding refused: {e}")).await;
                return;
            }
        };
        if !send(TcpInput::Connected).await {
            return;
        }
        let (mut read, mut write) = tokio::io::split(channel.into_stream());
        let mut buf = vec![0u8; 16 * 1024];
        let reason = loop {
            tokio::select! {
                result = read.read(&mut buf) => match result {
                    Ok(0) => break "closed by the device".to_string(),
                    Ok(n) => {
                        if !send(TcpInput::Data(buf[..n].to_vec())).await {
                            return;
                        }
                    }
                    Err(e) => break e.to_string(),
                },
                data = outgoing.recv() => match data {
                    Some(data) => {
                        if let Err(e) = write.write_all(&data).await {
                            break e.to_string();
                        }
                    }
                    None => return,
                },
            }
        };
        closed(reason).await;
    });
    crate::tcp::Connection {
        generation,
        writer,
        task,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use russh::keys::ssh_key::private::Ed25519Keypair;
    use russh::keys::PrivateKey;
    use russh::server::{self, Auth, Msg, Session};
    use russh::Channel;
    use tokio::net::{TcpListener, TcpStream};

    use super::*;

    /// An SSH server that accepts admin / s3cret and forwards direct-tcpip
    /// channels to wherever they ask.
    struct Server;

    impl server::Handler for Server {
        type Error = russh::Error;

        async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
            Ok(if user == "admin" && password == "s3cret" {
                Auth::Accept
            } else {
                Auth::reject()
            })
        }

        async fn channel_open_direct_tcpip(
            &mut self,
            channel: Channel<Msg>,
            host: &str,
            port: u32,
            _originator: &str,
            _originator_port: u32,
            reply: server::ChannelOpenHandle,
            _session: &mut Session,
        ) -> Result<(), Self::Error> {
            let target = format!("{host}:{port}");
            reply.accept().await;
            tokio::spawn(async move {
                let Ok(mut tcp) = TcpStream::connect(target).await else {
                    return;
                };
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
            });
            Ok(())
        }
    }

    fn host_key() -> PrivateKey {
        PrivateKey::from(Ed25519Keypair::from_seed(&[7; 32]))
    }

    /// The SSH server, counting the SSH connections it accepts.
    async fn ssh_server() -> (SocketAddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = Arc::new(server::Config {
            keys: vec![host_key()],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::from_millis(0)),
            ..Default::default()
        });
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                let config = config.clone();
                tokio::spawn(async move {
                    if let Ok(session) = server::run_stream(config, stream, Server).await {
                        let _ = session.await;
                    }
                });
            }
        });
        (addr, accepted)
    }

    /// A service behind the SSH server that echoes what it receives.
    async fn echo() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let (mut r, mut w) = stream.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        port
    }

    fn tunnel(
        ssh: SocketAddr,
        password: &str,
        fingerprint: Option<String>,
        port: u16,
    ) -> SshTunnel {
        SshTunnel {
            ssh,
            username: "admin".into(),
            password: password.into(),
            fingerprint,
            cipher: Some("aes128-ctr".into()),
            target_host: "127.0.0.1".into(),
            target_port: port,
        }
    }

    async fn next(rx: &mut mpsc::Receiver<Inbound>) -> TcpInput {
        match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(Inbound::Tcp { input, .. })) => input,
            other => panic!("expected a TCP event, got {}", other.is_ok()),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn streams_through_one_shared_ssh_session() {
        let (ssh, accepted) = ssh_server().await;
        let port = echo().await;
        let fingerprint = host_key()
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
        let sessions = Sessions::default();
        let (tx, mut rx) = mpsc::channel(64);

        let first = spawn(
            "a",
            1,
            tunnel(ssh, "s3cret", Some(fingerprint.clone()), port),
            sessions.clone(),
            tx.clone(),
        );
        assert_eq!(next(&mut rx).await, TcpInput::Connected);
        first.writer.send(b"hello".to_vec()).unwrap();
        assert_eq!(next(&mut rx).await, TcpInput::Data(b"hello".to_vec()));

        // A second stream to the same device reuses the SSH session.
        let second = spawn(
            "b",
            2,
            tunnel(ssh, "s3cret", Some(fingerprint), port),
            sessions,
            tx,
        );
        assert_eq!(next(&mut rx).await, TcpInput::Connected);
        second.writer.send(b"again".to_vec()).unwrap();
        assert_eq!(next(&mut rx).await, TcpInput::Data(b"again".to_vec()));
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_wrong_password_or_host_key_is_a_refusal() {
        let (ssh, _) = ssh_server().await;
        let port = echo().await;
        let (tx, mut rx) = mpsc::channel(64);

        let _c = spawn(
            "a",
            1,
            tunnel(ssh, "wrong", None, port),
            Sessions::default(),
            tx.clone(),
        );
        match next(&mut rx).await {
            TcpInput::Closed { reason } => assert!(reason.starts_with(REFUSED), "{reason}"),
            other => panic!("{other:?}"),
        }

        let other = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
        let _c = spawn(
            "b",
            2,
            tunnel(ssh, "s3cret", Some(other), port),
            Sessions::default(),
            tx,
        );
        match next(&mut rx).await {
            TcpInput::Closed { reason } => {
                assert!(reason.starts_with(REFUSED), "{reason}");
                assert!(
                    reason.contains("SHA256:"),
                    "names the presented key: {reason}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn fingerprints_compare_as_openssh_prints_them() {
        let presented = "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU";
        assert!(same_fingerprint(presented, presented));
        assert!(same_fingerprint(
            "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=",
            presented
        ));
        assert!(!same_fingerprint("SHA256:AAAA", presented));
    }
}
