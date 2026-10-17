use std::path::{Path, PathBuf};

use orrin_core::model::Artifact;
use orrin_core::{Hash, Home, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub outputs: Vec<Artifact>,
}

pub struct Records {
    deps: PathBuf,
    results: PathBuf,
}

impl Records {
    pub fn open(home: &Home) -> Result<Self> {
        Ok(Self { deps: home.dir("builds/steps/deps")?, results: home.dir("builds/steps/results")? })
    }

    pub fn deps(&self, base: &Hash) -> Option<Vec<String>> {
        read(&self.deps.join(base.as_str()))
    }

    pub fn put_deps(&self, base: &Hash, deps: &[String]) -> Result<()> {
        write(&self.deps.join(base.as_str()), &deps)
    }

    pub fn outcome(&self, key: &Hash) -> Option<Outcome> {
        read(&self.results.join(key.as_str()))
    }

    pub fn put_outcome(&self, key: &Hash, outcome: &Outcome) -> Result<()> {
        write(&self.results.join(key.as_str()), outcome)
    }

    pub fn roots(&self) -> Vec<Hash> {
        let files = std::fs::read_dir(&self.results).into_iter().flatten().flatten();
        let outcomes = files.filter_map(|entry| read::<Outcome>(&entry.path()));
        outcomes.flat_map(|outcome| outcome.outputs.into_iter().map(|a| a.hash)).collect()
    }
}

fn read<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let temp = path.with_extension(orrin_core::id::fresh("tmp"));
    std::fs::write(&temp, serde_json::to_vec(value)?)?;
    std::fs::rename(temp, path)?;
    Ok(())
}
