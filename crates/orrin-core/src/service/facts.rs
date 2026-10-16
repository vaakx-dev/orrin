use async_trait::async_trait;

use crate::api::{FactsOutput, FactsSet};
use crate::error::Result;
use crate::model::Tool;
use crate::service::Service;

#[async_trait]
pub trait Facts: Service {
    async fn list(&self, input: crate::api::Facts) -> Result<FactsOutput>;
    async fn set(&self, input: FactsSet) -> Result<Tool>;
    async fn tool(&self, name: &str) -> Result<Option<Tool>>;
    async fn learn(&self, name: &str, path: &str, version: Option<String>) -> Result<()>;
}
