use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::{ChangeId, Hash, ProjectId, SnapshotId, ViewId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    User,
    Worker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewState {
    Open,
    Landed,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub id: ViewId,
    pub project: ProjectId,
    pub owner: String,
    pub kind: ViewKind,
    pub path: PathBuf,
    pub base: SnapshotId,
    #[serde(default)]
    pub base_view: Option<ViewId>,
    pub claims: Vec<String>,
    pub state: ViewState,
    pub created_at: Millis,
    pub landed: Option<ChangeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub path: String,
    pub base: Option<Hash>,
    pub ours: Option<Hash>,
    pub theirs: Option<Hash>,
    pub reason: String,
}
