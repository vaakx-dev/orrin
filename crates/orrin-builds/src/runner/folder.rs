use std::path::{Path, PathBuf};

use orrin_core::model::{Entry, Mode, clean_path};
use orrin_core::par::par_map;
use orrin_core::service::{Materialise, Store};
use orrin_core::{JobId, Result};

pub const SCREENSHOTS: &str = ".orrin/screenshots";

pub fn fresh(runs: &Path, id: &JobId) -> Result<PathBuf> {
    let dir = runs.join(id.as_str());
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(dir.join(SCREENSHOTS))?;
    Ok(dir)
}

pub fn fill(store: &dyn Store, dir: &Path, files: &[Entry], big: u64) -> Result<()> {
    let placed = par_map(files, |entry| {
        let how = if entry.size > big { Materialise::Link } else { Materialise::Copy };
        write(store, dir, entry, how).map(drop)
    });
    placed.into_iter().collect()
}

pub fn write(store: &dyn Store, dir: &Path, entry: &Entry, how: Materialise) -> Result<PathBuf> {
    let target = dir.join(clean_path(&entry.path)?);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    store.materialise(&entry.hash, &target, how)?;
    if matches!(entry.mode, Mode::Executable) {
        executable(&target)?;
    }
    Ok(target)
}

pub fn remove(dir: &Path) {
    if let Err(err) = std::fs::remove_dir_all(dir) {
        tracing::warn!("could not remove run folder {}: {err}", dir.display());
    }
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
