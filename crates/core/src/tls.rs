//! TLS for native modules' TCP streams and for WebSockets: rustls with the
//! ring provider, TLS 1.2 or later, and the webpki roots reqwest trusts.
//!
//! A device with a self-signed certificate is reached with
//! `accept_invalid_certs`: the stream is still encrypted, and the handshake's
//! signatures are still checked against the certificate presented, but the
//! certificate itself is not, so the device is not authenticated.

use std::sync::{Arc, OnceLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};

/// The prefix of every `Closed` reason a TLS failure produces.
pub const PREFIX: &str = "tls:";

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// The client configuration: shared, built once for each mode.
pub(crate) fn client_config(accept_invalid_certs: bool) -> Arc<ClientConfig> {
    static VERIFIED: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    static UNVERIFIED: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    let cell = if accept_invalid_certs {
        &UNVERIFIED
    } else {
        &VERIFIED
    };
    cell.get_or_init(|| Arc::new(build(accept_invalid_certs)))
        .clone()
}

fn build(accept_invalid_certs: bool) -> ClientConfig {
    let provider = provider();
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .expect("ring supports TLS 1.2 and 1.3");
    if accept_invalid_certs {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
            .with_no_client_auth()
    } else {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        builder.with_root_certificates(roots).with_no_client_auth()
    }
}

/// The name a certificate is checked against: a DNS name or an IP address.
pub(crate) fn server_name(name: &str) -> Result<ServerName<'static>, String> {
    let bare = name.trim_start_matches('[').trim_end_matches(']');
    ServerName::try_from(bare.to_string())
        .map_err(|_| format!("{PREFIX} '{name}' is not a valid server name"))
}

/// A handshake error as a `Closed` reason, saying what to do about an
/// untrusted certificate.
pub(crate) fn describe(e: &std::io::Error) -> String {
    let certificate = e
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Error>())
        .is_some_and(|inner| matches!(inner, Error::InvalidCertificate(_)));
    if certificate {
        format!(
            "{PREFIX} the device's certificate was rejected ({e}); a device with a self-signed \
             certificate needs accept_invalid_certs"
        )
    } else {
        format!("{PREFIX} {e}")
    }
}

/// Accepts any certificate, still checking that the handshake was signed by
/// the key in the certificate presented.
#[derive(Debug)]
struct AnyCertificate(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A local TLS server with a self-signed certificate.

    use std::sync::Arc;

    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::ServerConfig;

    /// A server configuration for `localhost` and 127.0.0.1, self-signed.
    pub(crate) fn server_config() -> Arc<ServerConfig> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
            .unwrap();
        let der = CertificateDer::from(cert.cert.der().to_vec());
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        Arc::new(
            ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(vec![der], key)
                .unwrap(),
        )
    }
}
