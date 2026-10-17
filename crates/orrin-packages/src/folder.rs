use std::path::Path;

use orrin_core::model::{Entry, Mode, Snapshot, clean_path};
use orrin_core::par::par_map;
use orrin_core::service::{Materialise, Store};
use orrin_core::{Error, Result};
use walkdir::WalkDir;

pub fn snapshot(store: &dyn Store, dir: &Path) -> Result<Snapshot> {
    let files: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    let entries = par_map(&files, |file| {
        let relative = file.path().strip_prefix(dir).map_err(|err| Error::internal(err.to_string()))?;
        let meta = file.metadata().map_err(|err| Error::internal(err.to_string()))?;
        Ok(Entry {
            path: relative.to_string_lossy().replace('\\', "/"),
            hash: store.put_file(file.path())?,
            size: meta.len(),
            mode: mode(&meta),
        })
    });
    Ok(Snapshot::new(entries.into_iter().collect::<Result<_>>()?))
}

pub fn fill(store: &dyn Store, dir: &Path, snapshot: &Snapshot) -> Result<()> {
    let placed = par_map(&snapshot.entries, |entry| {
        let target = dir.join(clean_path(&entry.path)?);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        store.materialise(&entry.hash, &target, Materialise::Link)
    });
    placed.into_iter().collect()
}

#[cfg(unix)]
fn mode(meta: &std::fs::Metadata) -> Mode {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o111 != 0 { Mode::Executable } else { Mode::File }
}

#[cfg(not(unix))]
fn mode(_: &std::fs::Metadata) -> Mode {
    Mode::File
}
