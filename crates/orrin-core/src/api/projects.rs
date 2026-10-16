use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::id::{ChangeId, PcId, ProjectId};
use crate::model::{Change, Project, ProjectSettings, View};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsList {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsListOutput {
    pub projects: Vec<Project>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsCreate {
    pub name: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub home_pc: Option<PcId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsImport {
    pub path: PathBuf,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectOpened {
    pub project: Project,
    pub view: Option<View>,
    pub files: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsSettings {
    pub project: ProjectId,
    #[serde(default)]
    pub set: Option<ProjectSettings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
    pub project: ProjectId,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub before: Option<ChangeId>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogOutput {
    pub changes: Vec<Change>,
}
