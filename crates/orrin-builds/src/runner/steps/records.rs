use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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

    pub fn used(&self, base: &Hash, key: &Hash) {
        for path in [self.deps.join(base.as_str()), self.results.join(key.as_str())] {
            if let Ok(file) = std::fs::File::options().write(true).open(path) {
                let _ = file.set_modified(SystemTime::now());
            }
        }
    }

    pub fn roots(&self) -> Vec<Hash> {
        let files = std::fs::read_dir(&self.results).into_iter().flatten().flatten();
        let outcomes = files.filter_map(|entry| read::<Outcome>(&entry.path()));
        outcomes.flat_map(|outcome| outcome.outputs.into_iter().map(|a| a.hash)).collect()
    }

    pub fn clean(&self, keep: Duration) -> u32 {
        let cutoff = SystemTime::now().checked_sub(keep).unwrap_or(SystemTime::UNIX_EPOCH);
        let mut removed = 0;
        for dir in [&self.deps, &self.results] {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let old = entry.metadata().and_then(|meta| meta.modified()).is_ok_and(|at| at < cutoff);
                if old && std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_records_not_used_lately() {
        let dir = tempfile::tempdir().unwrap();
        let records = Records::open(&Home::at(dir.path()).unwrap()).unwrap();
        let (old, kept) = ((Hash::of(b"old"), Hash::of(b"old-key")), (Hash::of(b"kept"), Hash::of(b"kept-key")));
        for (base, key) in [&old, &kept] {
            records.put_deps(base, &["a.h".to_string()]).unwrap();
            records.put_outcome(key, &Outcome::default()).unwrap();
        }
        std::thread::sleep(Duration::from_millis(50));
        records.used(&kept.0, &kept.1);
        assert_eq!(records.clean(Duration::from_millis(25)), 2);
        assert!(records.deps(&old.0).is_none() && records.outcome(&old.1).is_none());
        assert!(records.deps(&kept.0).is_some() && records.outcome(&kept.1).is_some());
    }
}
