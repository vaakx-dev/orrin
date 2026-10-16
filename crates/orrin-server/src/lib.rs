mod http;
mod registry;
mod routes;
mod wiring;

use std::net::SocketAddr;
use std::sync::Arc;

use orrin_core::daemon::DaemonInfo;
use orrin_core::service::Hub;
use orrin_core::{Config, Home, Result, now_ms};
use tokio::net::TcpListener;

pub use http::auth::{KEY_HEADER, PC_HEADER};
pub use registry::Registry;
pub use wiring::wire;

pub struct Server {
    hub: Hub,
    listener: TcpListener,
}

impl Server {
    pub async fn bind(home: Home, port: Option<u16>) -> Result<Self> {
        let config = Config::load(&home)?;
        let port = port.unwrap_or(config.server.port);
        let listener = TcpListener::bind((config.server.bind.as_str(), port)).await?;
        let hub = wire(home, config)?;
        Ok(Self { hub, listener })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    pub async fn run(self, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
        let info =
            DaemonInfo { port: self.port(), pid: std::process::id(), pc: self.hub.me.pc.clone(), started_at: now_ms() };
        info.write(&self.hub.home)?;
        tracing::info!(port = info.port, home = %self.hub.home.root().display(), "orrin serving");
        let state = http::AppState { hub: self.hub.clone(), registry: Arc::new(routes::registry()) };
        let app = http::router(state).into_make_service_with_connect_info::<SocketAddr>();
        let served = axum::serve(self.listener, app).with_graceful_shutdown(shutdown).await;
        DaemonInfo::remove(&self.hub.home);
        Ok(served?)
    }
}
