use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use orrin_core::{ChangeId, Hash, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    #[serde(skip)]
    path: PathBuf,
    links: Vec<Link>,
    blobs: BTreeMap<Hash, String>,
    objects: BTreeMap<String, Hash>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Link {
    change: ChangeId,
    commit: String,
}

impl Mapping {
    pub fn load(path: &Path) -> Result<Self> {
        let mut mapping = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(err) => return Err(err.into()),
        };
        mapping.path = path.to_path_buf();
        Ok(mapping)
    }

    pub fn save(&self) -> Result<()> {
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }

    pub fn commit_of(&self, change: &ChangeId) -> Option<&str> {
        self.links.iter().rev().find(|l| &l.change == change).map(|l| l.commit.as_str())
    }

    pub fn change_of(&self, commit: &str) -> Option<&ChangeId> {
        self.links.iter().rev().find(|l| l.commit == commit).map(|l| &l.change)
    }

    pub fn link(&mut self, change: ChangeId, commit: String) {
        self.links.push(Link { change, commit });
    }

    pub fn blob(&self, hash: &Hash) -> Option<&str> {
        self.blobs.get(hash).map(String::as_str)
    }

    pub fn object(&self, oid: &str) -> Option<&Hash> {
        self.objects.get(oid)
    }

    pub fn remember(&mut self, hash: Hash, oid: String) {
        self.objects.insert(oid.clone(), hash.clone());
        self.blobs.insert(hash, oid);
    }
}
