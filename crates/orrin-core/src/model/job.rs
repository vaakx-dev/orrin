use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::{Hash, JobId, PcId, ProjectId, SnapshotId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Build,
    Run,
    Screenshot,
    Test,
    Export,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_done(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: JobId,
    pub project: ProjectId,
    pub kind: JobKind,
    pub recipe: String,
    pub snapshot: SnapshotId,
    pub pc: PcId,
    pub state: JobState,
    pub cached: bool,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub created_at: Millis,
    pub started_at: Option<Millis>,
    pub finished_at: Option<Millis>,
    #[serde(default)]
    pub result: Option<Hash>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobResult {
    pub exit_code: Option<i32>,
    pub artifacts: Vec<Artifact>,
    pub logs: Vec<LogLine>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogLine {
    pub n: u64,
    pub stream: Stream,
    pub text: String,
    pub at: Millis,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub hash: Hash,
    pub size: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Manifest {
    pub ignore: Vec<String>,
    pub recipes: BTreeMap<String, Recipe>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recipe {
    pub inputs: Vec<String>,
    pub command: String,
    pub outputs: Vec<String>,
    pub needs: Vec<String>,
    pub cpu: Option<u32>,
    pub ram_mb: Option<u64>,
    pub gpu: bool,
    pub after: Vec<String>,
    pub cache: CompilerCache,
    pub warm: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerCache {
    #[default]
    None,
    Sccache,
    Ccache,
    Scons,
}

pub const MANIFEST_PATH: &str = "orrin.toml";
