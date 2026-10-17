use std::path::Path;
use std::time::Duration;

use orrin_core::Result;
use orrin_core::api::PcsSettings;

use crate::service::{CacheService, blocking};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const GB: u64 = 1 << 30;

impl CacheService {
    pub(crate) async fn drop_unused(&self) -> Result<u64> {
        let keep = DAY * self.config.cache.keep_days;
        let mut over = self.over_space().await?;
        let mut keys = self.keys.all();
        keys.sort_by_key(|key| key.used);
        let mut dropped = 0;
        for key in keys {
            let old = key.used.elapsed().is_ok_and(|age| age > keep);
            if !old && over == 0 {
                break;
            }
            over = over.saturating_sub(self.hub.store.size(&key.hash).unwrap_or(0));
            self.keys.forget(&key.name);
            dropped += 1;
        }
        Ok(dropped)
    }

    async fn over_space(&self) -> Result<u64> {
        let mine = PcsSettings { pc: None, set: None };
        let settings = self.hub.peers.settings(mine).await.unwrap_or_else(|_| self.config.pc.clone());
        let Some(limit) = settings.space_gb.map(|gb| gb * GB) else { return Ok(0) };
        let store = self.home.store();
        let used = blocking(move || Ok(used_bytes(&store))).await?;
        Ok(used.saturating_sub(limit))
    }
}

fn used_bytes(dir: &Path) -> u64 {
    let files = walkdir::WalkDir::new(dir).into_iter().flatten().filter_map(|entry| entry.metadata().ok());
    files.filter(|meta| meta.is_file()).map(|meta| meta.len()).sum()
}
