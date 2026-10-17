use orrin_core::api::{PcsList, SyncPackage};
use orrin_core::service::{Services, call_peer};
use orrin_core::{Hash, PcId, Result, SnapshotId};

pub async fn fetch(hub: &Services, key: &Hash) -> Option<(PcId, SnapshotId)> {
    let pcs = hub.peers.pcs(PcsList {}).await.ok()?.pcs;
    for pc in pcs.into_iter().filter(|pc| !pc.me && pc.online) {
        let Ok(found) = call_peer(&*hub.peers, &pc.id, SyncPackage { key: key.clone() }).await else { continue };
        let Some(snapshot) = found.snapshot else { continue };
        match pull(hub, &pc.id, &snapshot).await {
            Ok(()) => return Some((pc.id, snapshot)),
            Err(err) => tracing::warn!("could not copy package {key} from {}: {err}", pc.id),
        }
    }
    None
}

async fn pull(hub: &Services, pc: &PcId, snapshot: &SnapshotId) -> Result<()> {
    hub.peers.pull(pc, vec![snapshot.clone()]).await?;
    let entries = hub.store.snapshot(snapshot)?.entries;
    hub.peers.pull(pc, entries.into_iter().map(|e| e.hash).collect()).await
}
