use serde::{Deserialize, Serialize};

use crate::id::ProjectId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deliver {
    pub project: ProjectId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryStatus {
    pub project: ProjectId,
}
