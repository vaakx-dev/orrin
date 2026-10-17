use async_trait::async_trait;

use crate::api::{CacheEntry, SyncCache};
use crate::error::Result;
use crate::id::Hash;
use crate::service::Service;

#[async_trait]
pub trait Cache: Service {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<()>;
    async fn find(&self, input: SyncCache) -> Result<CacheEntry>;
    async fn trim(&self) -> Result<u64>;
    async fn roots(&self) -> Result<Vec<Hash>>;
}
