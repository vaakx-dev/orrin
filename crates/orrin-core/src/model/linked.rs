use serde::{Deserialize, Serialize};

use crate::id::SnapshotId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Linked {
    pub target: String,
    pub files: SnapshotId,
}
