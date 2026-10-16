use async_trait::async_trait;

use crate::api::{
    Find, FindOutput, Outline, OutlineOutput, Read, ReadOutput, Search, SearchOutput, Where, WhereOutput,
};
use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::id::{ProjectId, ViewId};
use crate::service::Service;

#[async_trait]
pub trait Index: Service {
    async fn find(&self, input: Find) -> Result<FindOutput>;
    async fn search(&self, input: Search) -> Result<SearchOutput>;
    async fn outline(&self, input: Outline) -> Result<OutlineOutput>;
    async fn locate(&self, input: Where) -> Result<WhereOutput>;
    async fn touched(&self, project: &ProjectId, view: Option<&ViewId>, paths: &[String]) -> Result<Clock>;
    async fn clock(&self, project: &ProjectId, view: Option<&ViewId>) -> Clock;
    async fn read_anchor(&self, _input: Read) -> Result<ReadOutput> {
        Err(Error::unavailable("document text is unavailable"))
    }
}
