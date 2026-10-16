use std::path::Path;

use orrin_core::Result;
use orrin_core::model::{Artifact, Snapshot};
use orrin_core::service::Store;
use walkdir::WalkDir;

use super::folder::SCREENSHOTS;
use crate::recipe;

pub fn collect(store: &dyn Store, dir: &Path, patterns: &[String], snapshot: &Snapshot) -> Result<Vec<Artifact>> {
    let globs = recipe::globs(patterns)?;
    let mut artifacts = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        let Ok(relative) = entry.path().strip_prefix(dir) else { continue };
        let path = relative.to_string_lossy().replace('\\', "/");
        let screenshot = path.starts_with(SCREENSHOTS);
        if !screenshot && !globs.is_match(&path) {
            continue;
        }
        let hash = store.put_file(entry.path())?;
        if snapshot.get(&path).is_some_and(|e| e.hash == hash) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or_default();
        artifacts.push(Artifact { path, hash, size });
    }
    Ok(artifacts)
}
