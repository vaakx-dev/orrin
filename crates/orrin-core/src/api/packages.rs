use serde::{Deserialize, Serialize};

use crate::id::{Hash, SnapshotId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncPackage {
    pub key: Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFound {
    pub snapshot: Option<SnapshotId>,
}
