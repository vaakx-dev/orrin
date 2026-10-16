use serde::{Deserialize, Serialize};

use crate::id::PcId;
use crate::model::Tool;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    #[serde(default)]
    pub pc: Option<PcId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactsOutput {
    pub pc: PcId,
    pub tools: Vec<Tool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactsSet {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub version: Option<String>,
}
