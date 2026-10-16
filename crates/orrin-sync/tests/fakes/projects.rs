use std::sync::Mutex;

use async_trait::async_trait;
use orrin_core::api::{Log, LogOutput, ProjectOpened, ProjectsCreate, ProjectsListOutput, ProjectsSettings};
use orrin_core::model::{Change, Project, ProjectSettings};
use orrin_core::service::{Projects, Service};
use orrin_core::{ChangeId, Error, ProjectId, Result};

#[derive(Default)]
pub struct MemoryProjects {
    projects: Mutex<Vec<Project>>,
    changes: Mutex<Vec<Change>>,
}

impl Service for MemoryProjects {}

#[async_trait]
impl Projects for MemoryProjects {
    async fn list(&self) -> Result<ProjectsListOutput> {
        Ok(ProjectsListOutput { projects: self.projects.lock().unwrap().clone() })
    }

    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> {
        Err(Error::unavailable("fake"))
    }

    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> {
        Err(Error::unavailable("fake"))
    }

    async fn log(&self, input: Log) -> Result<LogOutput> {
        let changes = self.changes.lock().unwrap().iter().filter(|c| c.project == input.project).cloned().collect();
        Ok(LogOutput { changes })
    }

    async fn get(&self, id: &ProjectId) -> Result<Project> {
        let projects = self.projects.lock().unwrap();
        projects.iter().find(|p| &p.id == id).cloned().ok_or_else(|| Error::not_found(id.to_string()))
    }

    async fn save(&self, project: Project) -> Result<()> {
        let mut projects = self.projects.lock().unwrap();
        projects.retain(|p| p.id != project.id);
        projects.push(project);
        Ok(())
    }

    async fn swap_main(&self, _: Change) -> Result<Project> {
        Err(Error::unavailable("fake"))
    }

    async fn append(&self, changes: Vec<Change>) -> Result<()> {
        self.changes.lock().unwrap().extend(changes);
        Ok(())
    }

    async fn change(&self, id: &ChangeId) -> Result<Change> {
        let changes = self.changes.lock().unwrap();
        changes.iter().find(|c| &c.id == id).cloned().ok_or_else(|| Error::not_found(id.to_string()))
    }

    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> {
        Ok(())
    }

    async fn thin(&self, _: &ProjectId) -> Result<u32> {
        Ok(0)
    }
}
