use std::collections::{HashMap, HashSet};
use std::fs::{File, Metadata};
use std::path::Path;
use std::time::SystemTime;

use orrin_core::model::{FileState, Mode};
use orrin_core::service::Store;
use orrin_core::{Error, Hash, Result};

#[derive(Clone)]
struct Stat {
    size: u64,
    modified: Option<SystemTime>,
    state: FileState,
}

#[derive(Default)]
pub struct Stats(HashMap<String, Stat>);

impl Stats {
    pub fn state(&mut self, rel: &str, abs: &Path, meta: &Metadata) -> Result<FileState> {
        if let Some(stat) = self.0.get(rel).filter(|s| matches(s, meta)) {
            return Ok(stat.state.clone());
        }
        let state = FileState { hash: hash_file(abs)?, size: meta.len(), mode: mode_of(meta) };
        self.remember(rel, meta, state.hash.clone());
        Ok(state)
    }

    pub fn disk_state(&mut self, rel: &str, abs: &Path) -> Result<Option<FileState>> {
        match std::fs::symlink_metadata(abs) {
            Ok(meta) if meta.is_file() => Ok(Some(self.state(rel, abs, &meta)?)),
            Ok(_) => Ok(None),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn remember(&mut self, rel: &str, meta: &Metadata, hash: Hash) -> FileState {
        let state = FileState { hash, size: meta.len(), mode: mode_of(meta) };
        self.0.insert(rel.to_string(), Stat { size: meta.len(), modified: meta.modified().ok(), state: state.clone() });
        state
    }

    pub fn remember_path(&mut self, rel: &str, abs: &Path, hash: Hash) -> Result<()> {
        let meta = std::fs::metadata(abs)?;
        self.remember(rel, &meta, hash);
        Ok(())
    }

    pub fn remember_put(&mut self, rel: &str, abs: &Path, store: &dyn Store) -> Result<FileState> {
        let meta = std::fs::metadata(abs)?;
        let hash = store.put_file(abs)?;
        Ok(self.remember(rel, &meta, hash))
    }

    pub fn unchanged(&self, rel: &str, meta: &Metadata) -> bool {
        self.0.get(rel).is_some_and(|s| matches(s, meta))
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(stat) = self.0.remove(from) {
            self.0.insert(to.to_string(), stat);
        }
    }

    pub fn forget(&mut self, rel: &str) {
        self.0.remove(rel);
    }

    pub fn forget_under(&mut self, rel: &str) -> Vec<String> {
        let prefix = format!("{rel}/");
        let gone: Vec<String> = self.0.keys().filter(|k| *k == rel || k.starts_with(&prefix)).cloned().collect();
        gone.iter().for_each(|k| self.forget(k));
        gone
    }

    pub fn keep_only(&mut self, seen: &HashSet<String>) {
        self.0.retain(|k, _| seen.contains(k));
    }
}

fn matches(stat: &Stat, meta: &Metadata) -> bool {
    stat.size == meta.len() && stat.modified.is_some() && stat.modified == meta.modified().ok()
}

fn hash_file(path: &Path) -> Result<Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?).map_err(Error::from)?;
    Ok(Hash::from_blake3(hasher.finalize()))
}

#[cfg(unix)]
fn mode_of(meta: &Metadata) -> Mode {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o111 != 0 { Mode::Executable } else { Mode::File }
}

#[cfg(not(unix))]
fn mode_of(_meta: &Metadata) -> Mode {
    Mode::File
}
