use std::path::Path;

use async_trait::async_trait;
use serde_json::Value;

use crate::api::*;
use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::id::{ChangeId, Hash, PcId, ProjectId, SnapshotId, ViewId};
use crate::model::{Change, Job, JobKind, Linked, Pc, PcSettings, Project, ProjectSettings, Tool, View};
use crate::service::{
    Builds, Cache, Edits, Facts, GcReport, Git, Index, Landing, Materialise, PackageNeed, PackageReady, Packages,
    Peers, Projects, Service, Store, Views,
};

#[derive(Clone, Copy, Debug)]
pub struct Unavailable(pub &'static str);

impl Unavailable {
    fn err<T>(&self) -> Result<T> {
        Err(Error::unavailable(format!("{} is not built yet", self.0)))
    }
}

impl Service for Unavailable {}

impl Store for Unavailable {
    fn put(&self, _: &[u8]) -> Result<Hash> { self.err() }
    fn put_file(&self, _: &Path) -> Result<Hash> { self.err() }
    fn get(&self, _: &Hash) -> Result<Vec<u8>> { self.err() }
    fn size(&self, _: &Hash) -> Result<u64> { self.err() }
    fn has(&self, _: &Hash) -> bool { false }
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> { hashes.to_vec() }
    fn materialise(&self, _: &Hash, _: &Path, _: Materialise) -> Result<()> { self.err() }
    fn gc(&self, _: &[Hash]) -> Result<GcReport> { self.err() }
    fn parts(&self, _: &Hash) -> Result<Vec<Hash>> { self.err() }
    fn raw(&self, _: &Hash) -> Result<Vec<u8>> { self.err() }
    fn put_raw(&self, _: &Hash, _: &[u8]) -> Result<()> { self.err() }
}

#[async_trait]
impl Projects for Unavailable {
    async fn list(&self) -> Result<ProjectsListOutput> { self.err() }
    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> { self.err() }
    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> { self.err() }
    async fn log(&self, _: Log) -> Result<LogOutput> { self.err() }
    async fn get(&self, _: &ProjectId) -> Result<Project> { self.err() }
    async fn save(&self, _: Project) -> Result<()> { self.err() }
    async fn swap_main(&self, _: Change) -> Result<Project> { self.err() }
    async fn append(&self, _: Vec<Change>) -> Result<()> { self.err() }
    async fn change(&self, _: &ChangeId) -> Result<Change> { self.err() }
    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> { self.err() }
    async fn thin(&self, _: &ProjectId) -> Result<u32> { self.err() }
}

#[async_trait]
impl Views for Unavailable {
    async fn import(&self, _: ProjectsImport) -> Result<ProjectOpened> { self.err() }
    async fn create(&self, _: ViewCreate) -> Result<View> { self.err() }
    async fn list(&self, _: ViewList) -> Result<ViewListOutput> { self.err() }
    async fn remove(&self, _: ViewDrop) -> Result<Done> { self.err() }
    async fn read(&self, _: Read) -> Result<ReadOutput> { self.err() }
    async fn patch(&self, _: Patch) -> Result<Written> { self.err() }
    async fn create_file(&self, _: Create) -> Result<Written> { self.err() }
    async fn move_file(&self, _: Move) -> Result<Touched> { self.err() }
    async fn delete(&self, _: Delete) -> Result<Touched> { self.err() }
    async fn changes(&self, _: Changes) -> Result<ChangesOutput> { self.err() }
    async fn diff(&self, _: Diff) -> Result<DiffOutput> { self.err() }
    async fn get(&self, _: &ViewId) -> Result<View> { self.err() }
    async fn adopt(&self, _: &ProjectId, _: &Path) -> Result<View> { self.err() }
    async fn capture(&self, _: &ViewId) -> Result<Edits> { self.err() }
    async fn advance(&self, _: &ViewId, _: &SnapshotId) -> Result<Vec<String>> { self.err() }
    async fn landed(&self, _: &ViewId, _: &ChangeId) -> Result<()> { self.err() }
    async fn link(&self, _: &ProjectId, _: Linked) -> Result<()> { Ok(()) }
}

#[async_trait]
impl Landing for Unavailable {
    async fn land(&self, _: ViewLand) -> Result<LandOutput> { self.err() }
    async fn undo(&self, _: Undo) -> Result<LandOutput> { self.err() }
    async fn land_change(&self, _: SyncLand) -> Result<LandOutput> { self.err() }
}

#[async_trait]
impl Index for Unavailable {
    async fn find(&self, _: Find) -> Result<FindOutput> { self.err() }
    async fn search(&self, _: Search) -> Result<SearchOutput> { self.err() }
    async fn outline(&self, _: Outline) -> Result<OutlineOutput> { self.err() }
    async fn locate(&self, _: Where) -> Result<WhereOutput> { self.err() }
    async fn touched(&self, _: &ProjectId, _: Option<&ViewId>, _: &[String]) -> Result<Clock> { Ok(Clock::default()) }
    async fn clock(&self, _: &ProjectId, _: Option<&ViewId>) -> Clock { Clock::default() }
}

#[async_trait]
impl Facts for Unavailable {
    async fn list(&self, _: crate::api::Facts) -> Result<FactsOutput> { self.err() }
    async fn set(&self, _: FactsSet) -> Result<Tool> { self.err() }
    async fn tool(&self, _: &str) -> Result<Option<Tool>> { self.err() }
    async fn learn(&self, _: &str, _: &str, _: Option<String>) -> Result<()> { Ok(()) }
}

#[async_trait]
impl Builds for Unavailable {
    async fn submit(&self, _: JobKind, _: JobInput) -> Result<Job> { self.err() }
    async fn accept(&self, _: SyncJob) -> Result<Job> { self.err() }
    async fn status(&self, _: JobRef) -> Result<Job> { self.err() }
    async fn logs(&self, _: JobLogs) -> Result<JobLogsOutput> { self.err() }
    async fn artifacts(&self, _: JobRef) -> Result<JobArtifactsOutput> { self.err() }
    async fn cancel(&self, _: JobRef) -> Result<Job> { self.err() }
    async fn list(&self, _: JobList) -> Result<JobListOutput> { self.err() }
    async fn roots(&self) -> Result<Vec<Hash>> { Ok(Vec::new()) }
    async fn linked(&self, _: &ProjectId) -> Result<Vec<Linked>> { Ok(Vec::new()) }
}

#[async_trait]
impl Cache for Unavailable {
    async fn get(&self, _: &str) -> Result<Option<Vec<u8>>> { Ok(None) }
    async fn put(&self, _: &str, _: Vec<u8>) -> Result<()> { self.err() }
    async fn find(&self, _: SyncCache) -> Result<CacheEntry> { Ok(CacheEntry::default()) }
    async fn trim(&self) -> Result<u64> { Ok(0) }
    async fn roots(&self) -> Result<Vec<Hash>> { Ok(Vec::new()) }
}

#[async_trait]
impl Packages for Unavailable {
    async fn key(&self, _: PackageNeed) -> Result<Hash> { self.err() }
    async fn ensure(&self, _: PackageNeed) -> Result<PackageReady> { self.err() }
    async fn find(&self, _: SyncPackage) -> Result<PackageFound> { Ok(PackageFound { snapshot: None }) }
    async fn clean(&self) -> Result<u32> { Ok(0) }
    async fn roots(&self) -> Result<Vec<Hash>> { Ok(Vec::new()) }
}

#[async_trait]
impl Peers for Unavailable {
    async fn pcs(&self, _: PcsList) -> Result<PcsListOutput> { self.err() }
    async fn pair(&self, _: PcsPair) -> Result<PcsPairOutput> { self.err() }
    async fn redeem(&self, _: PcsRedeem) -> Result<Pc> { self.err() }
    async fn settings(&self, _: PcsSettings) -> Result<PcSettings> { self.err() }
    async fn accept_pair(&self, _: SyncPair) -> Result<SyncPairOutput> { self.err() }
    async fn announce(&self, _: SyncAnnounce) -> Result<SyncAnnounce> { self.err() }
    async fn missing(&self, _: SyncMissing) -> Result<Hashes> { self.err() }
    async fn objects(&self, _: SyncGet) -> Result<Objects> { self.err() }
    async fn receive(&self, _: SyncPut) -> Result<Stored> { self.err() }
    async fn changes(&self, _: SyncChanges) -> Result<SyncChangesOutput> { self.err() }
    async fn poke(&self, _: SyncPoke) -> Result<Done> { self.err() }
    async fn authorize(&self, _: &PcId, _: &str) -> bool { false }
    async fn call(&self, _: &PcId, _: &str, _: Value) -> Result<Value> { self.err() }
    async fn pull(&self, _: &PcId, _: Vec<Hash>) -> Result<()> { self.err() }
    async fn push(&self, _: &PcId, _: Vec<Hash>) -> Result<()> { self.err() }
    async fn refresh(&self, _: &PcId, _: &ProjectId) -> Result<()> { self.err() }
}

#[async_trait]
impl Git for Unavailable {
    async fn export(&self, _: GitExport) -> Result<GitExportOutput> { self.err() }
    async fn import(&self, _: GitImport) -> Result<GitImportOutput> { self.err() }
}
