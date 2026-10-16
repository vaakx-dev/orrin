use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::id::{ChangeId, Hash, ProjectId, SnapshotId, ViewId};
use crate::model::{Encoding, FileStatus};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub path: String,
    #[serde(default)]
    pub offset: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub encoding: Option<Encoding>,
    #[serde(default)]
    pub anchor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadOutput {
    pub clock: Clock,
    pub path: String,
    pub hash: Hash,
    pub size: u64,
    pub binary: bool,
    pub total_lines: Option<u32>,
    pub encoding: Encoding,
    pub content: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    pub old: String,
    pub new: String,
    #[serde(default)]
    pub all: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub project: ProjectId,
    pub view: ViewId,
    pub path: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub project: ProjectId,
    pub view: ViewId,
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub encoding: Encoding,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Written {
    pub clock: Clock,
    pub path: String,
    pub hash: Hash,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Move {
    pub project: ProjectId,
    pub view: ViewId,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delete {
    pub project: ProjectId,
    pub view: ViewId,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Touched {
    pub clock: Clock,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Changes {
    pub project: ProjectId,
    pub view: ViewId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedFile {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangesOutput {
    pub base: SnapshotId,
    pub files: Vec<ChangedFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diff {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    #[serde(default)]
    pub change: Option<ChangeId>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub binary: bool,
    pub patch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffOutput {
    pub files: Vec<FileDiff>,
}
