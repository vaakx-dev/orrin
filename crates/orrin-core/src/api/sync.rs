use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::JobLogs;
use crate::id::{ChangeId, Hash, JobId, PcId, ProjectId, SnapshotId};
use crate::model::{Change, FileState, JobKind, PcSettings, Project};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncPair {
    pub code: String,
    pub pc: PcId,
    pub name: String,
    pub address: String,
    pub key: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncPairOutput {
    pub pc: PcId,
    pub name: String,
    pub key: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncAnnounce {
    pub pc: PcId,
    pub name: String,
    #[serde(default)]
    pub address: Option<String>,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub settings: PcSettings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hashes {
    pub hashes: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncMissing(pub Hashes);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncGet(pub Hashes);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub hash: Hash,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Objects {
    pub objects: Vec<Object>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncPut(pub Objects);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    pub stored: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncChanges {
    pub project: ProjectId,
    #[serde(default)]
    pub after: Option<ChangeId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncChangesOutput {
    pub project: Project,
    pub changes: Vec<Change>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncPoke {
    pub pc: PcId,
    pub project: ProjectId,
    pub main: SnapshotId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncLand {
    pub project: ProjectId,
    pub parent: SnapshotId,
    pub files: BTreeMap<String, Option<FileState>>,
    pub author: String,
    pub intent: String,
    #[serde(default)]
    pub claims: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncJob {
    pub kind: JobKind,
    pub project: ProjectId,
    pub snapshot: SnapshotId,
    pub target: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub job: Option<JobId>,
    #[serde(default)]
    pub cancel: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncLogs(pub JobLogs);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncFacts {}
