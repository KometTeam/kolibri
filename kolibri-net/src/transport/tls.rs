use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use tokio_rustls::TlsConnector;

use super::error::TransportError;

/// Минцифры Root + Sub CA, the anchors Max endpoints chain to; absent from the
/// Mozilla bundle. Extracted from the Max app; see the file header.
const MINCIFRY_CA_PEM: &str = include_str!("mincifry_ca.pem");

/// process-wide opt-in to the bundled Минцифры CA, off by default. set once at
/// startup; read by every TLS path (socket, media, ws2).
static TRUST_MINCIFRY: AtomicBool = AtomicBool::new(false);

pub fn set_trust_mincifry_ca(enabled: bool) {
    TRUST_MINCIFRY.store(enabled, Ordering::Relaxed);
}

pub fn trust_mincifry_ca() -> bool {
    TRUST_MINCIFRY.load(Ordering::Relaxed)
}

/// Mozilla roots, plus the Минцифры CA when the flag is on (additive: rustls
/// picks the matching anchor per connection).
fn root_store() -> Result<RootCertStore, TransportError> {
    let mut roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    if trust_mincifry_ca() {
        for cert in rustls_pemfile::certs(&mut MINCIFRY_CA_PEM.as_bytes()) {
            let cert = cert.map_err(|e| TransportError::Tls(format!("mincifry CA parse: {e}")))?;
            roots
                .add(cert)
                .map_err(|e| TransportError::Tls(format!("mincifry CA add: {e}")))?;
        }
    }
    Ok(roots)
}

/// Shared client config for every TLS path. `insecure` accepts any cert
/// (self-signed / MitM-debug only).
pub fn build_client_config(insecure: bool) -> Result<Arc<ClientConfig>, TransportError> {
    let provider = Arc::new(ring::default_provider());

    let config = if insecure {
        ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|e| TransportError::Tls(e.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert(provider)))
            .with_no_client_auth()
    } else {
        ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| TransportError::Tls(e.to_string()))?
            .with_root_certificates(root_store()?)
            .with_no_client_auth()
    };

    Ok(Arc::new(config))
}

/// TLS connector for the main socket and media uploads. For OS-trust-store
/// parity, swap the root store for `rustls-platform-verifier`.
pub fn build_connector(insecure: bool) -> Result<TlsConnector, TransportError> {
    Ok(TlsConnector::from(build_client_config(insecure)?))
}

/// Accepts every cert. DEBUG ONLY, wide open to MitM.
#[derive(Debug)]
struct AcceptAnyCert(Arc<CryptoProvider>);

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
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
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
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
mod tests {
    use super::*;

    #[test]
    fn mincifry_flag_adds_two_anchors() {
        let bundled = rustls_pemfile::certs(&mut MINCIFRY_CA_PEM.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .expect("bundle parses");
        assert_eq!(bundled.len(), 2, "root + sub CA");

        set_trust_mincifry_ca(false);
        let base = root_store().unwrap().roots.len();
        assert_eq!(base, webpki_roots::TLS_SERVER_ROOTS.len());

        set_trust_mincifry_ca(true);
        assert!(trust_mincifry_ca());
        let with_ca = root_store().unwrap().roots.len();
        assert_eq!(
            with_ca,
            base + 2,
            "Минцифры anchors added on top of Mozilla"
        );

        assert!(build_client_config(false).is_ok());
        assert!(build_client_config(true).is_ok());

        set_trust_mincifry_ca(false);
    }
}
