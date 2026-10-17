use std::path::{Path, PathBuf};

use orrin_core::model::{Artifact, Entry, clean_path};
use orrin_core::service::{Materialise, Store};
use orrin_core::{Error, Hash, Result};

use crate::paths;
use crate::runner::folder;

pub fn depfile(out: &str) -> String {
    format!("{out}.d")
}

pub fn clear(dir: &Path, out: Option<&str>, also: &[String]) -> Result<()> {
    let mut stale = extras(dir, also)?;
    if let Some(out) = out {
        stale.extend([out.to_string(), depfile(out)]);
        if let Some(parent) = inside(dir, out)?.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    for path in stale {
        let file = inside(dir, &path)?;
        if file.exists() {
            std::fs::remove_file(file)?;
        }
    }
    Ok(())
}

pub fn keep(store: &dyn Store, dir: &Path, out: Option<&str>, also: &[String]) -> Result<Vec<Artifact>> {
    let mut kept = Vec::new();
    if let Some(out) = out {
        let file = inside(dir, out)?;
        let meta = std::fs::metadata(&file).map_err(|_| Error::invalid(format!("the step did not write {out}")))?;
        kept.push(artifact(store, &file, out.to_string(), &meta)?);
    }
    for path in extras(dir, also)?.into_iter().filter(|path| Some(path.as_str()) != out) {
        let file = inside(dir, &path)?;
        let meta = std::fs::metadata(&file)?;
        kept.push(artifact(store, &file, path, &meta)?);
    }
    Ok(kept)
}

fn artifact(store: &dyn Store, file: &Path, path: String, meta: &std::fs::Metadata) -> Result<Artifact> {
    Ok(Artifact { hash: store.put_file(file)?, path, size: meta.len(), modified: paths::modified(meta) })
}

fn extras(dir: &Path, also: &[String]) -> Result<Vec<String>> {
    if also.is_empty() {
        return Ok(Vec::new());
    }
    let globs = paths::globs(also)?;
    Ok(paths::walk(dir, also).into_iter().filter(|path| globs.is_match(path)).collect())
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
