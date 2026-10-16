use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::connect_info::Connected;
use axum::serve::IncomingStream;
use orrin_core::cert::Cert;
use orrin_core::{Error, Result};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;
use tokio_util::either::Either;

const TLS_HANDSHAKE: u8 = 0x16;
const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

pub type Stream = Either<TcpStream, TlsStream<TcpStream>>;

pub fn install_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[derive(Clone, Copy, Debug)]
pub struct Remote(pub SocketAddr);

impl Remote {
    pub fn is_loopback(&self) -> bool {
        self.0.ip().to_canonical().is_loopback()
    }
}

impl Connected<IncomingStream<'_, Listener>> for Remote {
    fn connect_info(stream: IncomingStream<'_, Listener>) -> Self {
        *stream.remote_addr()
    }
}

pub struct Listener {
    local: SocketAddr,
    opened: mpsc::Receiver<(Stream, Remote)>,
    accepting: JoinHandle<()>,
}

impl Listener {
    pub fn new(tcp: TcpListener, cert: &Cert) -> Result<Self> {
        let local = tcp.local_addr()?;
        let tls = TlsAcceptor::from(Arc::new(server_config(cert)?));
        let (sender, opened) = mpsc::channel(64);
        let accepting = tokio::spawn(accept_all(tcp, tls, sender));
        Ok(Self { local, opened, accepting })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

impl axum::serve::Listener for Listener {
    type Io = Stream;
    type Addr = Remote;

    async fn accept(&mut self) -> (Stream, Remote) {
        match self.opened.recv().await {
            Some(opened) => opened,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Remote> {
        Ok(Remote(self.local))
    }
}

async fn accept_all(tcp: TcpListener, tls: TlsAcceptor, opened: mpsc::Sender<(Stream, Remote)>) {
    loop {
        let (stream, addr) = match tcp.accept().await {
            Ok(accepted) => accepted,
            Err(err) => {
                tracing::debug!(%err, "accept failed");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let (tls, opened) = (tls.clone(), opened.clone());
        tokio::spawn(async move {
            let remote = Remote(addr);
            if let Ok(Some(stream)) = tokio::time::timeout(OPEN_TIMEOUT, open(stream, remote, tls)).await {
                let _ = opened.send((stream, remote)).await;
            }
        });
    }
}

async fn open(stream: TcpStream, remote: Remote, tls: TlsAcceptor) -> Option<Stream> {
    let mut first = [0u8; 1];
    if stream.peek(&mut first).await.ok()? == 0 {
        return None;
    }
    if first[0] == TLS_HANDSHAKE {
        return tls.accept(stream).await.ok().map(Either::Right);
    }
    remote.is_loopback().then_some(Either::Left(stream))
}

fn server_config(cert: &Cert) -> Result<ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key.clone()));
    ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(|b| b.with_no_client_auth().with_single_cert(vec![CertificateDer::from(cert.der.clone())], key))
        .map_err(|e| Error::internal(format!("tls setup: {e}")))
}
