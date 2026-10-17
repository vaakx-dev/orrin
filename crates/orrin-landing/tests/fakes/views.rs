use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::{Artifact, Delivered, Linked, View, ViewState};
use orrin_core::service::{Edits, Service, Views};
use orrin_core::{ChangeId, Error, ProjectId, Result, SnapshotId, ViewId};

#[derive(Default)]
pub struct FakeViews {
    pub views: Mutex<BTreeMap<ViewId, (View, Edits)>>,
    pub busy: Mutex<Vec<String>>,
}

impl FakeViews {
    pub fn add(&self, view: View, edits: Edits) {
        self.views.lock().unwrap().insert(view.id.clone(), (view, edits));
    }

    pub fn view(&self, id: &ViewId) -> View {
        self.views.lock().unwrap()[id].0.clone()
    }

    fn unsupported<T>() -> Result<T> {
        Err(Error::unavailable("fake"))
    }
}

impl Service for FakeViews {}

#[async_trait]
impl Views for FakeViews {
    async fn import(&self, _: ProjectsImport) -> Result<ProjectOpened> {
        Self::unsupported()
    }

    async fn create(&self, _: ViewCreate) -> Result<View> {
        Self::unsupported()
    }

    async fn list(&self, input: ViewList) -> Result<ViewListOutput> {
        let views = self.views.lock().unwrap();
        let views = views.values().map(|(v, _)| v.clone());
        Ok(ViewListOutput { views: views.filter(|v| input.project.as_ref().is_none_or(|p| *p == v.project)).collect() })
    }

    async fn remove(&self, _: ViewDrop) -> Result<Done> {
        Self::unsupported()
    }

    async fn read(&self, _: Read) -> Result<ReadOutput> {
        Self::unsupported()
    }

    async fn patch(&self, _: Patch) -> Result<Written> {
        Self::unsupported()
    }

    async fn create_file(&self, _: Create) -> Result<Written> {
        Self::unsupported()
    }

    async fn move_file(&self, _: Move) -> Result<Touched> {
        Self::unsupported()
    }

    async fn delete(&self, _: Delete) -> Result<Touched> {
        Self::unsupported()
    }

    async fn changes(&self, _: Changes) -> Result<ChangesOutput> {
        Self::unsupported()
    }

    async fn diff(&self, _: Diff) -> Result<DiffOutput> {
        Self::unsupported()
    }

    async fn get(&self, id: &ViewId) -> Result<View> {
        let views = self.views.lock().unwrap();
        views.get(id).map(|(v, _)| v.clone()).ok_or_else(|| Error::not_found(id.to_string()))
    }

    async fn adopt(&self, _: &ProjectId, _: &Path) -> Result<View> {
        Self::unsupported()
    }

    async fn capture(&self, id: &ViewId) -> Result<Edits> {
        Ok(self.views.lock().unwrap()[id].1.clone())
    }

    async fn advance(&self, id: &ViewId, snapshot: &SnapshotId) -> Result<Vec<String>> {
        let mut views = self.views.lock().unwrap();
        let (view, edits) = views.get_mut(id).ok_or_else(|| Error::not_found(id.to_string()))?;
        view.base = snapshot.clone();
        edits.clear();
        Ok(self.busy.lock().unwrap().clone())
    }

    async fn take(&self, id: &ViewId, _: &SnapshotId, files: &Edits) -> Result<Vec<String>> {
        let mut views = self.views.lock().unwrap();
        let (_, edits) = views.get_mut(id).ok_or_else(|| Error::not_found(id.to_string()))?;
        edits.extend(files.clone());
        Ok(Vec::new())
    }

    async fn landed(&self, id: &ViewId, change: Option<&ChangeId>) -> Result<()> {
        let mut views = self.views.lock().unwrap();
        let (view, _) = views.get_mut(id).ok_or_else(|| Error::not_found(id.to_string()))?;
        view.state = ViewState::Landed;
        view.landed = change.cloned();
        Ok(())
    }

    async fn link(&self, _: &ProjectId, _: Linked) -> Result<()> {
        Ok(())
    }

    async fn deliver(&self, _: &ProjectId, _: Vec<Artifact>) -> Result<Delivered> {
        Ok(Delivered::default())
    }
}
