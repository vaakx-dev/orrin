use std::path::{Path, PathBuf};

use orrin_core::model::{Artifact, Entry, clean_path};
use orrin_core::service::{Materialise, Store};
use orrin_core::{Error, Hash, Result};

use crate::runner::folder;

pub fn depfile(out: &str) -> String {
    format!("{out}.d")
}

pub fn clear(dir: &Path, out: &str) -> Result<()> {
    let file = inside(dir, out)?;
    for path in [file.clone(), inside(dir, &depfile(out))?] {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn keep(store: &dyn Store, dir: &Path, out: Option<&str>) -> Result<Vec<Artifact>> {
    let Some(out) = out else { return Ok(Vec::new()) };
    let file = inside(dir, out)?;
    let meta = std::fs::metadata(&file).map_err(|_| Error::invalid(format!("the step did not write {out}")))?;
    Ok(vec![Artifact { path: out.to_string(), hash: store.put_file(&file)?, size: meta.len() }])
}

pub fn place(store: &dyn Store, dir: &Path, outputs: &[Artifact]) -> Result<()> {
    for artifact in outputs {
        let file = inside(dir, &artifact.path)?;
        let same = std::fs::metadata(&file).is_ok_and(|meta| meta.len() == artifact.size)
            && store.put_file(&file).is_ok_and(|hash| hash == artifact.hash);
        if !same {
            let entry = Entry {
                path: artifact.path.clone(),
                hash: artifact.hash.clone(),
                size: artifact.size,
                mode: Default::default(),
            };
            folder::write(store, dir, &entry, Materialise::Copy)?;
        }
    }
    Ok(())
}

pub fn dep_hashes(dir: &Path, deps: Vec<(String, Option<Hash>)>) -> Vec<(String, Hash)> {
    deps.into_iter()
        .map(|(path, known)| {
            let hash = known.unwrap_or_else(|| look(dir, &path));
            (path, hash)
        })
        .collect()
}

fn look(dir: &Path, path: &str) -> Hash {
    let outside = Path::new(path);
    let seen = match outside.is_absolute() {
        true => {
            std::fs::metadata(outside).map(|meta| format!("{}:{:?}", meta.len(), meta.modified().ok()).into_bytes())
        }
        false => std::fs::read(dir.join(path)),
    };
    Hash::of(&seen.unwrap_or_else(|_| b"missing".to_vec()))
}

fn inside(dir: &Path, path: &str) -> Result<PathBuf> {
    Ok(dir.join(clean_path(path)?))
}
