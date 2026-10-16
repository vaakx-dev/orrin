use async_trait::async_trait;

use crate::api::{GitExport, GitExportOutput, GitImport, GitImportOutput};
use crate::error::Result;
use crate::service::Service;

#[async_trait]
pub trait Git: Service {
    async fn export(&self, input: GitExport) -> Result<GitExportOutput>;
    async fn import(&self, input: GitImport) -> Result<GitImportOutput>;
}
