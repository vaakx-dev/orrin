mod content;
mod document;
mod layer;
mod main;
mod overlay;
mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use orrin_core::api::Event;
use orrin_core::clock::Ticker;
use orrin_core::service::Hub;
use orrin_core::{Clock, ProjectId, Result, SnapshotId, ViewId};

pub use content::blocking;
pub use state::Visible;

use crate::records::Records;
use state::ProjectState;

pub struct Engine {
    hub: Hub,
    records: Arc<Records>,
    projects: Mutex<HashMap<ProjectId, Arc<ProjectIndex>>>,
    ticker: Ticker,
}

#[derive(Default)]
pub struct ProjectIndex {
    gate: tokio::sync::Mutex<()>,
    state: RwLock<ProjectState>,
}

impl ProjectIndex {
    pub fn read(&self) -> RwLockReadGuard<'_, ProjectState> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, ProjectState> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    fn at(&self, main: &SnapshotId) -> bool {
        self.read().snapshot.as_ref() == Some(main)
    }

    fn has_overlay(&self, view: &ViewId) -> bool {
        self.read().overlays.contains_key(view)
    }
}

impl Engine {
    pub fn new(hub: Hub, records: Records) -> Self {
        let ticker = Ticker::starting_at(records.clock());
        Self { hub, records: Arc::new(records), projects: Mutex::default(), ticker }
    }

    pub fn hub(&self) -> &Hub {
        &self.hub
    }

    pub fn records(&self) -> Arc<Records> {
        self.records.clone()
    }

    pub async fn open(&self, project: &ProjectId, view: Option<&ViewId>) -> Result<Arc<ProjectIndex>> {
        let index = self.sync_main(project).await?;
        if let Some(view) = view {
            self.ensure_overlay(project, &index, view).await?;
        }
        Ok(index)
    }

    pub async fn touched(&self, project: &ProjectId, view: Option<&ViewId>, paths: &[String]) -> Result<Clock> {
        if let Some(view) = view
            && paths.is_empty()
        {
            return self.start_clean(project, view).await;
        }
        let before = self.index(project).read().clock;
        let index = self.sync_main(project).await?;
        let clock = match view {
            Some(view) => self.touch_view(project, &index, view, paths).await?,
            None if index.read().clock != before => return Ok(index.read().clock),
            None => {
                let clock = self.tick()?;
                index.write().clock = clock;
                clock
            }
        };
        self.announce(project, view, clock);
        Ok(clock)
    }

    pub async fn clock(&self, project: &ProjectId, view: Option<&ViewId>) -> Clock {
        let index = self.projects().get(project).cloned();
        index.map(|i| i.read().clock(view)).unwrap_or_default()
    }

    pub async fn warm(&self) -> Result<()> {
        let projects = self.hub.projects.list().await?.projects;
        for project in projects {
            if let Err(err) = self.sync_main(&project.id).await {
                tracing::warn!("indexing {} failed: {err}", project.id);
            }
        }
        Ok(())
    }

    pub fn forget(&self, view: &ViewId) {
        let indexes: Vec<Arc<ProjectIndex>> = self.projects().values().cloned().collect();
        for index in indexes {
            let mut state = index.write();
            state.overlays.remove(view);
            state.touches.remove(view);
        }
    }

    pub fn remove_project(&self, project: &ProjectId) {
        self.projects().remove(project);
    }

    fn index(&self, project: &ProjectId) -> Arc<ProjectIndex> {
        self.projects().entry(project.clone()).or_default().clone()
    }

    fn tick(&self) -> Result<Clock> {
        let clock = self.ticker.tick();
        self.records.save_clock(clock)?;
        Ok(clock)
    }

    fn announce(&self, project: &ProjectId, view: Option<&ViewId>, clock: Clock) {
        self.hub.events.emit(Event::IndexUpdated { project: project.clone(), view: view.cloned(), clock });
    }

    fn projects(&self) -> std::sync::MutexGuard<'_, HashMap<ProjectId, Arc<ProjectIndex>>> {
        self.projects.lock().unwrap_or_else(|e| e.into_inner())
    }
}
