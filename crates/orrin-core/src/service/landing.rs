use async_trait::async_trait;

use crate::api::{LandOutput, SyncLand, Undo, ViewLand};
use crate::error::Result;
use crate::service::Service;

#[async_trait]
pub trait Landing: Service {
    async fn land(&self, input: ViewLand) -> Result<LandOutput>;
    async fn undo(&self, input: Undo) -> Result<LandOutput>;
    async fn land_change(&self, input: SyncLand) -> Result<LandOutput>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Merged {
    Clean(Vec<u8>),
    Conflict(String),
}

pub trait Merger: Send + Sync {
    fn name(&self) -> &'static str;
    fn handles(&self, path: &str) -> bool;
    fn merge(&self, path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged;
}
