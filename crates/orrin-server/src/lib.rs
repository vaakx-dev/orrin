mod http;
mod maintenance;
mod registry;
mod routes;
mod transport;
mod wiring;

use std::sync::Arc;

use orrin_core::cert::Cert;
use orrin_core::daemon::DaemonInfo;
use orrin_core::service::Hub;
use orrin_core::{Config, Home, Result, now_ms};
use tokio::net::TcpListener;

pub use http::auth::{KEY_HEADER, PC_HEADER};
pub use registry::Registry;
pub use transport::install_crypto;
pub use wiring::wire;

pub struct Server {
    hub: Hub,
    listener: TcpListener,
    cert: Cert,
    local_key: String,
}

impl Server {
    pub async fn bind(home: Home, port: Option<u16>) -> Result<Self> {
        install_crypto();
        let config = Config::load(&home)?;
        let port = port.unwrap_or(config.server.port);
        let listener = TcpListener::bind((config.server.bind.as_str(), port)).await?;
        let local_key = home.ensure_local_key()?;
        let cert = Cert::load_or_create(&home)?;
        let hub = wire(home, config)?;
        Ok(Self { hub, listener, cert, local_key })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    pub async fn run(self, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
        let info =
            DaemonInfo { port: self.port(), pid: std::process::id(), pc: self.hub.me.pc.clone(), started_at: now_ms() };
        info.write(&self.hub.home)?;
        tracing::info!(port = info.port, home = %self.hub.home.root().display(), "orrin serving");
        let state = http::AppState {
            hub: self.hub.clone(),
            registry: Arc::new(routes::registry()),
            local_key: self.local_key.into(),
        };
        let app = http::router(state).into_make_service_with_connect_info::<transport::Remote>();
        let listener = transport::Listener::new(self.listener, &self.cert)?;
        let served = axum::serve(listener, app).with_graceful_shutdown(shutdown).await;
        DaemonInfo::remove(&self.hub.home);
        Ok(served?)
    }
}
