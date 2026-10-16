use std::path::{Path, PathBuf};

use orrin_core::model::{Artifact, Mode, Snapshot, clean_path};
use orrin_core::service::{Materialise, Store};
use orrin_core::{Hash, JobId, Result};

pub const SCREENSHOTS: &str = ".orrin/screenshots";

pub fn fresh(runs: &Path, id: &JobId) -> Result<PathBuf> {
    let dir = runs.join(id.as_str());
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(dir.join(SCREENSHOTS))?;
    Ok(dir)
}

pub fn fill(store: &dyn Store, dir: &Path, snapshot: &Snapshot, big: u64, extras: &[Artifact]) -> Result<()> {
    for entry in &snapshot.entries {
        let how = if entry.size > big { Materialise::Link } else { Materialise::Copy };
        let target = place(store, dir, &entry.path, &entry.hash, how)?;
        if entry.mode == Mode::Executable {
            executable(&target)?;
        }
    }
    for artifact in extras {
        place(store, dir, &artifact.path, &artifact.hash, Materialise::Copy)?;
    }
    Ok(())
}

pub fn remove(dir: &Path) {
    if let Err(err) = std::fs::remove_dir_all(dir) {
        tracing::warn!("could not remove run folder {}: {err}", dir.display());
    }
}

fn place(store: &dyn Store, dir: &Path, path: &str, hash: &Hash, how: Materialise) -> Result<PathBuf> {
    let target = dir.join(clean_path(path)?);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    store.materialise(hash, &target, how)?;
    Ok(target)
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    Ok(std::fs::set_permissions(path, perms)?)
}

#[cfg(not(unix))]
fn executable(_path: &Path) -> Result<()> {
    Ok(())
}
