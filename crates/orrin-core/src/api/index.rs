use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::id::{ProjectId, ViewId};
use crate::model::{FileHit, OutlineItem, SearchHit, Symbol};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Find {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub pattern: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindOutput {
    pub clock: Clock,
    pub files: Vec<FileHit>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub query: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub paths: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchOutput {
    pub clock: Clock,
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outline {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineOutput {
    pub clock: Clock,
    pub path: String,
    pub items: Vec<OutlineItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Where {
    pub project: ProjectId,
    #[serde(default)]
    pub view: Option<ViewId>,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhereOutput {
    pub clock: Clock,
    pub symbols: Vec<Symbol>,
}
