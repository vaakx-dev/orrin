use std::time::Duration;

use orrin_core::PcId;
use orrin_core::api::{CacheEntry, PcsList, SyncCache};
use orrin_core::service::call_peer;
use tokio::time::timeout;

use crate::service::CacheService;

const ASK: Duration = Duration::from_secs(3);
const FETCH: Duration = Duration::from_secs(120);

impl CacheService {
    pub(crate) async fn ask_peers(&self, key: &str) -> Option<Vec<u8>> {
        for pc in self.online_peers().await {
            let ask = call_peer(&*self.hub.peers, &pc, SyncCache { key: key.to_string() });
            let Ok(Ok(CacheEntry { hash: Some(hash) })) = timeout(ASK, ask).await else { continue };
            if !matches!(timeout(FETCH, self.hub.peers.pull(&pc, vec![hash.clone()])).await, Ok(Ok(()))) {
                continue;
            }
            if self.keys.put(key, &hash).is_ok()
                && let Some(bytes) = self.read(key, hash).await
            {
                return Some(bytes);
            }
        }
        None
    }

    async fn online_peers(&self) -> Vec<PcId> {
        let pcs = self.hub.peers.pcs(PcsList {}).await.map(|list| list.pcs).unwrap_or_default();
        pcs.into_iter().filter(|pc| !pc.me && pc.online).map(|pc| pc.id).collect()
    }
}
