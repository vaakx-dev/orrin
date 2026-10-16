use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::id::PcId;
use crate::model::{Pc, PcSettings};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsList {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsListOutput {
    pub pcs: Vec<Pc>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsPair {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsPairOutput {
    pub code: String,
    pub expires_at: Millis,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsRedeem {
    pub code: String,
    pub address: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcsSettings {
    #[serde(default)]
    pub pc: Option<PcId>,
    #[serde(default)]
    pub set: Option<PcSettings>,
}
