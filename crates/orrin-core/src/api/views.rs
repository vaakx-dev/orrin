use serde::{Deserialize, Serialize};

use crate::id::{ChangeId, ProjectId, SnapshotId, ViewId};
use crate::model::{Change, Conflict, View};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewCreate {
    pub project: ProjectId,
    pub owner: String,
    #[serde(default)]
    pub base: Option<SnapshotId>,
    #[serde(default)]
    pub claims: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewList {
    #[serde(default)]
    pub project: Option<ProjectId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewListOutput {
    pub views: Vec<View>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewLand {
    pub view: ViewId,
    pub intent: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDrop {
    pub view: ViewId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Undo {
    pub project: ProjectId,
    #[serde(default)]
    pub change: Option<ChangeId>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandOutput {
    pub landed: bool,
    pub change: Option<Change>,
    pub main: SnapshotId,
    pub conflicts: Vec<Conflict>,
    pub skipped: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Done {}
