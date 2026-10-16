use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Millis};
use crate::id::{ChangeId, JobId, ProjectId, SnapshotId, ViewId};
use crate::model::{Conflict, Job, LogLine, Pc, Project};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Event {
    #[serde(rename = "project.created")]
    ProjectCreated { project: Project },
    #[serde(rename = "main.moved")]
    MainMoved { project: ProjectId, main: SnapshotId, change: ChangeId },
    #[serde(rename = "view.changed")]
    ViewChanged { view: ViewId, paths: Vec<String>, clock: Clock },
    #[serde(rename = "view.landed")]
    ViewLanded { view: ViewId, change: ChangeId },
    #[serde(rename = "view.conflicts")]
    ViewConflicts { view: ViewId, conflicts: Vec<Conflict> },
    #[serde(rename = "view.skipped")]
    ViewSkipped { view: ViewId, paths: Vec<String> },
    #[serde(rename = "index.updated")]
    IndexUpdated { project: ProjectId, view: Option<ViewId>, clock: Clock },
    #[serde(rename = "job")]
    Job { job: Job },
    #[serde(rename = "job.log")]
    JobLog { job: JobId, line: LogLine },
    #[serde(rename = "pc.seen")]
    PcSeen { pc: Pc },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub seq: u64,
    pub at: Millis,
    pub event: Event,
}
