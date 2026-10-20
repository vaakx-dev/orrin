use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::{Artifact, Delivered, Linked, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Pulled, Service, Views};
use orrin_core::{ChangeId, Hash, ProjectId, Result, SnapshotId, ViewId};

#[derive(Default)]
pub struct FakeViews {
    pub delivered: Mutex<Vec<Vec<String>>>,
    pub busy: Mutex<Vec<String>>,
}

impl Service for FakeViews {}

fn user(project: ProjectId) -> View {
    View {
        id: ViewId::from("v_user"),
        project,
        owner: "me".into(),
        kind: ViewKind::User,
        path: "user".into(),
        base: Hash::of(b""),
        base_view: None,
        claims: Vec::new(),
        state: ViewState::Open,
        created_at: 0,
        landed: None,
    }
}

#[rustfmt::skip]
#[async_trait]
impl Views for FakeViews {
    async fn import(&self, _: ProjectsImport) -> Result<ProjectOpened> { unimplemented!() }
    async fn create(&self, _: ViewCreate) -> Result<View> { unimplemented!() }
    async fn list(&self, input: ViewList) -> Result<ViewListOutput> {
        Ok(ViewListOutput { views: input.project.into_iter().map(user).collect() })
    }
    async fn remove(&self, _: ViewDrop) -> Result<Done> { unimplemented!() }
    async fn read(&self, _: Read) -> Result<ReadOutput> { unimplemented!() }
    async fn patch(&self, _: Patch) -> Result<Written> { unimplemented!() }
    async fn create_file(&self, _: Create) -> Result<Written> { unimplemented!() }
    async fn move_file(&self, _: Move) -> Result<Touched> { unimplemented!() }
    async fn delete(&self, _: Delete) -> Result<Touched> { unimplemented!() }
    async fn changes(&self, _: Changes) -> Result<ChangesOutput> { unimplemented!() }
    async fn diff(&self, _: Diff) -> Result<DiffOutput> { unimplemented!() }
    async fn get(&self, _: &ViewId) -> Result<View> { unimplemented!() }
    async fn adopt(&self, _: &ProjectId, _: &Path) -> Result<View> { unimplemented!() }
    async fn capture(&self, _: &ViewId) -> Result<Edits> { unimplemented!() }
    async fn advance(&self, _: &ViewId, _: &SnapshotId) -> Result<Vec<String>> { unimplemented!() }
    async fn pull(&self, _: &ViewId, _: &SnapshotId, _: &Edits) -> Result<Pulled> { unimplemented!() }
    async fn take(&self, _: &ViewId, _: &SnapshotId, _: &Edits) -> Result<Vec<String>> { unimplemented!() }
    async fn landed(&self, _: &ViewId, _: Option<&ChangeId>) -> Result<()> { unimplemented!() }
    async fn link(&self, _: &ProjectId, _: Linked) -> Result<()> { Ok(()) }
    async fn deliver(&self, _: &ProjectId, files: Vec<Artifact>) -> Result<Delivered> {
        self.delivered.lock().unwrap().push(files.iter().map(|f| f.path.clone()).collect());
        let busy = std::mem::take(&mut *self.busy.lock().unwrap());
        Ok(Delivered { copied: (files.len() - busy.len()) as u32, unchanged: 0, busy })
    }
}
