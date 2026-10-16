use std::path::PathBuf;

use orrin_core::model::JobResult;
use orrin_core::service::Store;
use orrin_core::{Hash, Result};

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn lookup(&self, store: &dyn Store, key: &Hash) -> Option<(Hash, JobResult)> {
        let text = std::fs::read_to_string(self.dir.join(key.as_str())).ok()?;
        let hash = Hash::parse(text.trim()).ok()?;
        let result: JobResult = serde_json::from_slice(&store.get(&hash).ok()?).ok()?;
        result.artifacts.iter().all(|a| store.has(&a.hash)).then_some((hash, result))
    }

    pub fn put(&self, key: &Hash, result: &Hash) -> Result<()> {
        let path = self.dir.join(key.as_str());
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, result.as_str())?;
        std::fs::rename(temp, path)?;
        Ok(())
    }
}
