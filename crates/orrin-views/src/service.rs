use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orrin_core::api::{
    Changes, ChangesOutput, Create, Delete, Diff, DiffOutput, Done, Move, Patch, ProjectOpened, ProjectsImport, Read,
    ReadOutput, Touched, ViewCreate, ViewDrop, ViewList, ViewListOutput, Written,
};
use orrin_core::model::View;
use orrin_core::service::{Edits, Service, Views};
use orrin_core::{ChangeId, ProjectId, Result, SnapshotId, ViewId};

use crate::inner::Inner;
use crate::slot::Slot;
use crate::watch;

const PURGE_EVERY: Duration = Duration::from_secs(60 * 60);

pub struct ViewsService(pub Arc<Inner>);

impl Service for ViewsService {
    fn start(&self) -> Result<()> {
        for slot in self.0.slots() {
            if slot.record.try_lock().is_ok_and(|r| Slot::is_open(&r)) {
                watch::watch(&self.0, &slot);
            }
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return Ok(()) };
        if let Some(rx) = self.0.hits_rx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            runtime.spawn(watch::run(self.0.clone(), rx));
        }
        let inner = self.0.clone();
        runtime.spawn(async move {
            loop {
                if let Err(err) = inner.purge().await {
                    tracing::warn!("purging old views failed: {err}");
                }
                tokio::time::sleep(PURGE_EVERY).await;
            }
        });
        Ok(())
    }
}

#[async_trait]
impl Views for ViewsService {
    async fn import(&self, input: ProjectsImport) -> Result<ProjectOpened> {
        self.0.import(input).await
    }

    async fn create(&self, input: ViewCreate) -> Result<View> {
        self.0.create(input).await
    }

    async fn list(&self, input: ViewList) -> Result<ViewListOutput> {
        self.0.list(input).await
    }

    async fn remove(&self, input: ViewDrop) -> Result<Done> {
        self.0.remove(input).await
    }

    async fn read(&self, input: Read) -> Result<ReadOutput> {
        self.0.read(input).await
    }

    async fn patch(&self, input: Patch) -> Result<Written> {
        self.0.patch(input).await
    }

    async fn create_file(&self, input: Create) -> Result<Written> {
        self.0.create_file(input).await
    }

    async fn move_file(&self, input: Move) -> Result<Touched> {
        self.0.move_file(input).await
    }

    async fn delete(&self, input: Delete) -> Result<Touched> {
        self.0.delete(input).await
    }

    async fn changes(&self, input: Changes) -> Result<ChangesOutput> {
        self.0.changes(input).await
    }

    async fn diff(&self, input: Diff) -> Result<DiffOutput> {
        self.0.diff(input).await
    }

    async fn get(&self, id: &ViewId) -> Result<View> {
        self.0.get(id).await
    }

    async fn adopt(&self, project: &ProjectId, path: &Path) -> Result<View> {
        self.0.adopt(project, path).await
    }

    async fn capture(&self, id: &ViewId) -> Result<Edits> {
        self.0.capture(id).await
    }

    async fn advance(&self, id: &ViewId, snapshot: &SnapshotId) -> Result<Vec<String>> {
        self.0.advance(id, snapshot).await
    }

    async fn landed(&self, id: &ViewId, change: &ChangeId) -> Result<()> {
        self.0.landed(id, change).await
    }
}
