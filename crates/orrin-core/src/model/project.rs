use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::{ChangeId, PcId, ProjectId, SnapshotId};
use crate::model::files::FileState;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub home_pc: PcId,
    pub main: SnapshotId,
    pub settings: ProjectSettings,
    pub created_at: Millis,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectSettings {
    pub ignore: Vec<String>,
    pub git: Option<GitSettings>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSettings {
    pub remote: String,
    #[serde(default = "main_branch")]
    pub branch: String,
}

fn main_branch() -> String {
    "main".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub id: ChangeId,
    pub project: ProjectId,
    pub parent: SnapshotId,
    pub snapshot: SnapshotId,
    pub files: BTreeMap<String, Option<FileState>>,
    pub author: String,
    pub intent: String,
    pub at: Millis,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub undoes: Option<ChangeId>,
}
