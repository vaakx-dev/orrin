use async_trait::async_trait;

use crate::api::{JobArtifactsOutput, JobInput, JobLogs, JobLogsOutput, JobRef, SyncJob};
use crate::error::Result;
use crate::model::{Job, JobKind};
use crate::service::Service;

#[async_trait]
pub trait Builds: Service {
    async fn submit(&self, kind: JobKind, input: JobInput) -> Result<Job>;
    async fn accept(&self, input: SyncJob) -> Result<Job>;
    async fn status(&self, input: JobRef) -> Result<Job>;
    async fn logs(&self, input: JobLogs) -> Result<JobLogsOutput>;
    async fn artifacts(&self, input: JobRef) -> Result<JobArtifactsOutput>;
    async fn cancel(&self, input: JobRef) -> Result<Job>;
}
