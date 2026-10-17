use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::{JobId, ProjectId, SnapshotId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Building,
    Copying,
    Waiting,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub project: ProjectId,
    pub main: SnapshotId,
    pub targets: Vec<String>,
    pub state: DeliveryState,
    pub jobs: Vec<JobId>,
    pub copied: u32,
    pub unchanged: u32,
    pub waiting: Vec<String>,
    pub error: Option<String>,
    pub started_at: Millis,
    pub finished_at: Option<Millis>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered {
    pub copied: u32,
    pub unchanged: u32,
    pub busy: Vec<String>,
}
