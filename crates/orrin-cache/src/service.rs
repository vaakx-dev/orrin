use async_trait::async_trait;
use orrin_core::api::{CacheEntry, SyncCache};
use orrin_core::service::{Cache, Hub, Service, Setup};
use orrin_core::{Config, Error, Hash, Home, Result};

use crate::keys::{Keys, checked};

pub struct CacheService {
    pub(crate) hub: Hub,
    pub(crate) home: Home,
    pub(crate) config: Config,
    pub(crate) keys: Keys,
}

impl CacheService {
    pub fn open(setup: &Setup) -> Result<Self> {
        Ok(Self {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.clone(),
            keys: Keys::new(setup.home.dir("caches/orrin")?),
        })
    }

    pub(crate) async fn read(&self, key: &str, hash: Hash) -> Option<Vec<u8>> {
        let store = self.hub.store.clone();
        let bytes = blocking(move || store.get(&hash)).await.ok();
        if bytes.is_none() {
            self.keys.forget(key);
        }
        bytes
    }
}

impl Service for CacheService {}

#[async_trait]
impl Cache for CacheService {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        checked(key)?;
        if let Some(hash) = self.keys.find(key)
            && let Some(bytes) = self.read(key, hash).await
        {
            return Ok(Some(bytes));
        }
        Ok(self.ask_peers(key).await)
    }

    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<()> {
        checked(key)?;
        let store = self.hub.store.clone();
        let hash = blocking(move || store.put(&bytes)).await?;
        self.keys.put(key, &hash)
    }

    async fn find(&self, input: SyncCache) -> Result<CacheEntry> {
        let hash = self.keys.find(&input.key).filter(|hash| self.hub.store.has(hash));
        Ok(CacheEntry { hash })
    }

    async fn trim(&self) -> Result<u64> {
        self.drop_unused().await
    }

    async fn roots(&self) -> Result<Vec<Hash>> {
        Ok(self.keys.all().into_iter().map(|key| key.hash).collect())
    }
}

pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|err| Error::internal(err.to_string()))?
}
