use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::PcId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keep {
    #[default]
    All,
    Active,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PcSettings {
    pub keep: Keep,
    pub builds: bool,
    pub workers: u32,
    pub space_gb: Option<u64>,
}

impl Default for PcSettings {
    fn default() -> Self {
        Self { keep: Keep::All, builds: true, workers: 4, space_gb: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pc {
    pub id: PcId,
    pub name: String,
    pub address: Option<String>,
    pub me: bool,
    pub online: bool,
    pub last_seen: Option<Millis>,
    pub settings: PcSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Path,
    Registry,
    KnownDir,
    Learned,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    pub source: ToolSource,
}
