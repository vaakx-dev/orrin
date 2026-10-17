use std::path::Path;

use orrin_core::Result;
use orrin_core::model::{Artifact, Snapshot};
use orrin_core::service::Store;

use super::folder::SCREENSHOTS;
use crate::recipe;

pub fn collect(store: &dyn Store, dir: &Path, patterns: &[String], snapshot: &Snapshot) -> Result<Vec<Artifact>> {
    let globs = recipe::globs(patterns)?;
    let mut paths = recipe::walk(dir, patterns);
    paths.retain(|path| globs.is_match(path));
    paths.extend(recipe::walk(dir, &[format!("{SCREENSHOTS}/**")]));
    let mut artifacts = Vec::new();
    for path in paths {
        let file = dir.join(&path);
        let hash = store.put_file(&file)?;
        if snapshot.get(&path).is_some_and(|e| e.hash == hash) {
            continue;
        }
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or_default();
        artifacts.push(Artifact { path, hash, size });
    }
    Ok(artifacts)
}
