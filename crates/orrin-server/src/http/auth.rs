use std::net::SocketAddr;

use axum::http::HeaderMap;
use orrin_core::api::is_peer_request;
use orrin_core::service::Hub;
use orrin_core::{Error, PcId, Result};

pub const PC_HEADER: &str = "x-orrin-pc";
pub const KEY_HEADER: &str = "x-orrin-key";
const OPEN_TO_STRANGERS: &[&str] = &["sync.pair"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    Local,
    Peer(PcId),
    Stranger,
}

pub async fn identify(hub: &Hub, addr: SocketAddr, headers: &HeaderMap) -> Caller {
    if addr.ip().is_loopback() {
        return Caller::Local;
    }
    let header = |name| headers.get(name).and_then(|v| v.to_str().ok());
    match (header(PC_HEADER), header(KEY_HEADER)) {
        (Some(pc), Some(key)) if hub.peers.authorize(&PcId::from(pc), key).await => Caller::Peer(PcId::from(pc)),
        _ => Caller::Stranger,
    }
}

pub fn allow(caller: &Caller, request: &str) -> Result<()> {
    let allowed = match caller {
        Caller::Local => true,
        Caller::Peer(_) => is_peer_request(request),
        Caller::Stranger => OPEN_TO_STRANGERS.contains(&request),
    };
    if allowed { Ok(()) } else { Err(Error::unauthorized(format!("not allowed to call {request}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peers_only_reach_sync_requests() {
        let peer = Caller::Peer(PcId::from("pc_x"));
        assert!(allow(&Caller::Local, "read").is_ok());
        assert!(allow(&peer, "sync.get").is_ok());
        assert!(allow(&peer, "read").is_err());
        assert!(allow(&Caller::Stranger, "sync.pair").is_ok());
        assert!(allow(&Caller::Stranger, "sync.get").is_err());
    }
}
