use serde::{Deserialize, Serialize};

use crate::id::Hash;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectsGet {
    pub hash: Hash,
}
