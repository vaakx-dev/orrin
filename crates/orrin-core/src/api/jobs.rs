use serde::{Deserialize, Serialize};

use crate::id::{JobId, PcId, ProjectId, ViewId};
use crate::model::{Artifact, JobKind, LogLine};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobInput {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub recipe: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub pc: Option<PcId>,
}

macro_rules! job_request {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub JobInput);

        impl $name {
            pub fn kind(&self) -> JobKind {
                JobKind::$kind
            }
        }
    };
}

job_request!(Build, Build);
job_request!(Run, Run);
job_request!(Screenshot, Screenshot);
job_request!(Test, Test);
job_request!(Export, Export);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRef {
    pub job: JobId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobStatus(pub JobRef);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobArtifacts(pub JobRef);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobCancel(pub JobRef);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobLogs {
    pub job: JobId,
    #[serde(default)]
    pub after: Option<u64>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobLogsOutput {
    pub lines: Vec<LogLine>,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobArtifactsOutput {
    pub artifacts: Vec<Artifact>,
}
