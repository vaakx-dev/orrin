use serde::{Deserialize, Serialize};

use crate::id::PcId;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ping {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pong {
    pub pc: PcId,
    pub name: String,
    pub version: String,
    pub pid: u32,
}
