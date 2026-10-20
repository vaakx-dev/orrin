use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;

use crate::api::{
    Changes, ChangesOutput, Create, Delete, Diff, DiffOutput, Done, Move, Patch, ProjectOpened, ProjectsImport, Read,
    ReadOutput, Touched, ViewCreate, ViewDrop, ViewList, ViewListOutput, Written,
};
use crate::error::Result;
use crate::id::{ChangeId, ProjectId, SnapshotId, ViewId};
use crate::model::{Artifact, Delivered, FileState, Linked, View};
use crate::service::Service;

pub type Edits = BTreeMap<String, Option<FileState>>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pulled {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
}

#[async_trait]
pub trait Views: Service {
    async fn import(&self, input: ProjectsImport) -> Result<ProjectOpened>;
    async fn create(&self, input: ViewCreate) -> Result<View>;
    async fn list(&self, input: ViewList) -> Result<ViewListOutput>;
    async fn remove(&self, input: ViewDrop) -> Result<Done>;
    async fn read(&self, input: Read) -> Result<ReadOutput>;
    async fn patch(&self, input: Patch) -> Result<Written>;
    async fn create_file(&self, input: Create) -> Result<Written>;
    async fn move_file(&self, input: Move) -> Result<Touched>;
    async fn delete(&self, input: Delete) -> Result<Touched>;
    async fn changes(&self, input: Changes) -> Result<ChangesOutput>;
    async fn diff(&self, input: Diff) -> Result<DiffOutput>;
    async fn get(&self, id: &ViewId) -> Result<View>;
    async fn adopt(&self, project: &ProjectId, path: &Path) -> Result<View>;
    async fn capture(&self, id: &ViewId) -> Result<Edits>;
    async fn advance(&self, id: &ViewId, snapshot: &SnapshotId) -> Result<Vec<String>>;
    async fn pull(&self, id: &ViewId, snapshot: &SnapshotId, merged: &Edits) -> Result<Pulled>;
    async fn take(&self, id: &ViewId, expected: &SnapshotId, files: &Edits) -> Result<Vec<String>>;
    async fn landed(&self, id: &ViewId, change: Option<&ChangeId>) -> Result<()>;
    async fn link(&self, project: &ProjectId, linked: Linked) -> Result<()>;
    async fn deliver(&self, project: &ProjectId, files: Vec<Artifact>) -> Result<Delivered>;
}
