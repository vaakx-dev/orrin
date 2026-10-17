use std::path::Path;

use orrin_core::Result;
use orrin_core::model::{Artifact, Snapshot, Target};
use orrin_core::service::Store;

use super::folder::SCREENSHOTS;
use crate::paths as files;

pub fn collect(store: &dyn Store, dir: &Path, target: &Target, snapshot: &Snapshot) -> Result<Vec<Artifact>> {
    let (globs, scratch) = (files::globs(&target.outputs)?, files::globs(&target.scratch)?);
    let mut paths = files::walk(dir, &target.outputs);
    paths.retain(|path| globs.is_match(path) && !scratch.is_match(path));
    paths.extend(files::walk(dir, &[format!("{SCREENSHOTS}/**")]));
    let mut artifacts = Vec::new();
    for path in paths {
        let file = dir.join(&path);
        let hash = store.put_file(&file)?;
        if snapshot.get(&path).is_some_and(|e| e.hash == hash) {
            continue;
        }
        let meta = std::fs::metadata(&file)?;
        artifacts.push(Artifact { path, hash, size: meta.len(), modified: files::modified(&meta) });
    }
    Ok(artifacts)
}
