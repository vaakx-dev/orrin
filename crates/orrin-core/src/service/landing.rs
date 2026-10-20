use async_trait::async_trait;

use crate::api::{LandOutput, PullOutput, SyncLand, Undo, ViewLand, ViewPull};
use crate::error::Result;
use crate::service::Service;

#[async_trait]
pub trait Landing: Service {
    async fn land(&self, input: ViewLand) -> Result<LandOutput>;
    async fn undo(&self, input: Undo) -> Result<LandOutput>;
    async fn land_change(&self, input: SyncLand) -> Result<LandOutput>;
    async fn pull(&self, input: ViewPull) -> Result<PullOutput>;
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

    fn markers(&self, _base: &[u8], _ours: &[u8], _theirs: &[u8]) -> Option<Vec<u8>> {
        None
    }
}
