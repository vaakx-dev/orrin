use async_trait::async_trait;
use orrin_core::api::{GitExport, GitExportOutput, GitImport, GitImportOutput};
use orrin_core::service::{Git, Hub, Service, Setup};
use orrin_core::{Home, Result};
use tokio::sync::Mutex;

use crate::target::Target;

pub struct GitService {
    pub home: Home,
    pub hub: Hub,
    busy: Mutex<()>,
}

impl GitService {
    pub fn new(setup: &Setup) -> Self {
        Self { home: setup.home.clone(), hub: setup.hub.clone(), busy: Mutex::new(()) }
    }
}

impl Service for GitService {}

#[async_trait]
impl Git for GitService {
    async fn export(&self, input: GitExport) -> Result<GitExportOutput> {
        let _busy = self.busy.lock().await;
        let project = self.hub.projects.get(&input.project).await?;
        let target = Target::resolve(&self.home, &project, input.remote, input.branch)?;
        crate::export::run(&self.hub, project, target).await
    }

    async fn import(&self, input: GitImport) -> Result<GitImportOutput> {
        let _busy = self.busy.lock().await;
        let project = self.hub.projects.get(&input.project).await?;
        let target = Target::resolve(&self.home, &project, input.remote, input.branch)?;
        crate::import::run(&self.hub, project, target).await
    }
}
