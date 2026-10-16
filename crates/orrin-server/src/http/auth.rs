use axum::http::HeaderMap;
use orrin_core::api::is_peer_request;
use orrin_core::daemon::LOCAL_KEY_HEADER;
use orrin_core::secret::same;
use orrin_core::{Error, PcId, Result};

use crate::http::AppState;
use crate::transport::Remote;

pub const PC_HEADER: &str = "x-orrin-pc";
pub const KEY_HEADER: &str = "x-orrin-key";
const OPEN_TO_STRANGERS: &[&str] = &["sync.pair"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    Local,
    Peer(PcId),
    Stranger,
}

pub async fn identify(state: &AppState, remote: Remote, headers: &HeaderMap, local_key: Option<&str>) -> Caller {
    let header = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let local_key = local_key.or(header(LOCAL_KEY_HEADER));
    if remote.is_loopback() && local_key.is_some_and(|key| same(key, &state.local_key)) {
        return Caller::Local;
    }
    match (header(PC_HEADER), header(KEY_HEADER)) {
        (Some(pc), Some(key)) if state.hub.peers.authorize(&PcId::from(pc), key).await => Caller::Peer(PcId::from(pc)),
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

pub fn allow_local(caller: &Caller) -> Result<()> {
    match caller {
        Caller::Local => Ok(()),
        _ => Err(Error::unauthorized(format!("send the local key in {LOCAL_KEY_HEADER}"))),
    }
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
