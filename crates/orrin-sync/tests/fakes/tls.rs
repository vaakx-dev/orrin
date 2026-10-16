use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use orrin_core::Home;
use orrin_core::cert::Cert;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

pub struct TlsListener {
    tcp: TcpListener,
    tls: TlsAcceptor,
}

impl TlsListener {
    pub fn new(tcp: TcpListener, home: &Home) -> Self {
        let cert = Cert::load_or_create(home).unwrap();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key));
        let config = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![CertificateDer::from(cert.der)], key)
            .unwrap();
        Self { tcp, tls: TlsAcceptor::from(Arc::new(config)) }
    }
}

impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, SocketAddr) {
        loop {
            let Ok((tcp, addr)) = self.tcp.accept().await else { continue };
            if let Ok(stream) = self.tls.accept(tcp).await {
                return (stream, addr);
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }
}
