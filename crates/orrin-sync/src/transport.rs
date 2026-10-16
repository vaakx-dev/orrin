use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use orrin_core::cert::fingerprint;
use orrin_core::secret::same;
use orrin_core::{Error, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};

#[derive(Default)]
pub struct Transport {
    clients: Mutex<HashMap<String, reqwest::Client>>,
}

impl Transport {
    pub fn client(&self, fingerprint: &str) -> Result<reqwest::Client> {
        let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(client) = clients.get(fingerprint) {
            return Ok(client.clone());
        }
        let client = pinned_client(fingerprint)?;
        clients.insert(fingerprint.to_string(), client.clone());
        Ok(client)
    }
}

fn pinned_client(fingerprint: &str) -> Result<reqwest::Client> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Pinned { fingerprint: fingerprint.to_string(), provider: provider.clone() };
    let tls = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::internal(format!("tls setup: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .tls_backend_preconfigured(tls)
        .build()
        .map_err(|e| Error::internal(format!("http client: {e}")))
}

#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        if same(&fingerprint(end_entity), &self.fingerprint) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}
