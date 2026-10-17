use std::path::PathBuf;

use async_trait::async_trait;

use crate::api::{PackageFound, SyncPackage};
use crate::error::Result;
use crate::id::{Hash, ProjectId, SnapshotId};
use crate::model::Package;
use crate::service::Service;

#[derive(Clone, Debug)]
pub struct PackageNeed {
    pub project: ProjectId,
    pub name: String,
    pub package: Package,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageReady {
    pub key: Hash,
    pub snapshot: SnapshotId,
    pub dir: PathBuf,
    pub note: String,
}

#[async_trait]
pub trait Packages: Service {
    async fn key(&self, need: PackageNeed) -> Result<Hash>;
    async fn ensure(&self, need: PackageNeed) -> Result<PackageReady>;
    async fn find(&self, input: SyncPackage) -> Result<PackageFound>;
    async fn clean(&self) -> Result<u32>;
    async fn roots(&self) -> Result<Vec<Hash>>;
}
