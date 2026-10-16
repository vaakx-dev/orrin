use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::id::{Hash, SnapshotId};
use crate::model::files::Entry;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
}

impl Snapshot {
    pub fn new(mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.dedup_by(|a, b| a.path == b.path);
        Self { entries }
    }

    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.entries.binary_search_by(|e| e.path.as_str().cmp(path)).ok().map(|i| &self.entries[i])
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn id(&self) -> SnapshotId {
        Hash::of(&self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry { path: path.into(), hash: Hash::of(path.as_bytes()), size: 1, mode: Default::default() }
    }

    #[test]
    fn sorted_lookup_and_stable_id() {
        let a = Snapshot::new(vec![entry("b"), entry("a")]);
        let b = Snapshot::new(vec![entry("a"), entry("b")]);
        assert_eq!(a.id(), b.id());
        assert!(a.get("b").is_some());
        assert!(a.get("c").is_none());
        assert_eq!(Snapshot::from_bytes(&a.to_bytes()).unwrap(), a);
    }
}
