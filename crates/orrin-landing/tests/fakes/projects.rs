use std::sync::Mutex;

use async_trait::async_trait;
use orrin_core::api::{Log, LogOutput, ProjectOpened, ProjectsCreate, ProjectsListOutput, ProjectsSettings};
use orrin_core::model::{Change, Project, ProjectSettings};
use orrin_core::service::{Projects, Service};
use orrin_core::{ChangeId, Error, ProjectId, Result};

pub struct FakeProjects {
    pub project: Mutex<Project>,
    pub changes: Mutex<Vec<Change>>,
}

impl FakeProjects {
    pub fn new(project: Project) -> Self {
        Self { project: Mutex::new(project), changes: Mutex::new(Vec::new()) }
    }

    fn unsupported<T>() -> Result<T> {
        Err(Error::unavailable("fake"))
    }
}

impl Service for FakeProjects {}

#[async_trait]
impl Projects for FakeProjects {
    async fn list(&self) -> Result<ProjectsListOutput> {
        Self::unsupported()
    }

    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> {
        Self::unsupported()
    }

    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> {
        Self::unsupported()
    }

    async fn log(&self, _: Log) -> Result<LogOutput> {
        Ok(LogOutput { changes: self.changes.lock().unwrap().clone() })
    }

    async fn get(&self, _: &ProjectId) -> Result<Project> {
        Ok(self.project.lock().unwrap().clone())
    }

    async fn save(&self, project: Project) -> Result<()> {
        *self.project.lock().unwrap() = project;
        Ok(())
    }

    async fn swap_main(&self, change: Change) -> Result<Project> {
        let mut project = self.project.lock().unwrap();
        if project.main != change.parent {
            return Err(Error::conflict("main moved"));
        }
        project.main = change.snapshot.clone();
        self.changes.lock().unwrap().push(change);
        Ok(project.clone())
    }

    async fn append(&self, changes: Vec<Change>) -> Result<()> {
        self.changes.lock().unwrap().extend(changes);
        Ok(())
    }

    async fn change(&self, id: &ChangeId) -> Result<Change> {
        let changes = self.changes.lock().unwrap();
        changes.iter().find(|c| c.id == *id).cloned().ok_or_else(|| Error::not_found(id.to_string()))
    }

    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> {
        Self::unsupported()
    }

    async fn thin(&self, _: &ProjectId) -> Result<u32> {
        Self::unsupported()
    }
}
