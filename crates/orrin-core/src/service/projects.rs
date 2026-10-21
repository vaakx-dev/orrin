use async_trait::async_trait;

use crate::api::{
    Log, LogOutput, ProjectOpened, ProjectRemoved, ProjectsCreate, ProjectsListOutput, ProjectsRemove, ProjectsSettings,
};
use crate::error::{Error, Result};
use crate::id::{ChangeId, ProjectId};
use crate::model::{Change, Project, ProjectSettings};
use crate::service::Service;

#[async_trait]
pub trait Projects: Service {
    async fn list(&self) -> Result<ProjectsListOutput>;
    async fn create(&self, input: ProjectsCreate) -> Result<ProjectOpened>;
    async fn settings(&self, input: ProjectsSettings) -> Result<ProjectSettings>;
    async fn log(&self, input: Log) -> Result<LogOutput>;
    async fn get(&self, id: &ProjectId) -> Result<Project>;
    async fn save(&self, project: Project) -> Result<()>;
    async fn swap_main(&self, change: Change) -> Result<Project>;
    async fn append(&self, changes: Vec<Change>) -> Result<()>;
    async fn change(&self, id: &ChangeId) -> Result<Change>;
    async fn pin(&self, id: &ChangeId, pinned: bool) -> Result<()>;
    async fn thin(&self, project: &ProjectId) -> Result<u32>;
    async fn remove(&self, _input: ProjectsRemove) -> Result<ProjectRemoved> {
        Err(Error::unavailable("removing projects is unavailable"))
    }
}
