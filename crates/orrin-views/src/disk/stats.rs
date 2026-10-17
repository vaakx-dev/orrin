use std::collections::{HashMap, HashSet};
use std::fs::{File, Metadata};
use std::path::Path;
use std::time::SystemTime;

use orrin_core::model::{FileState, Mode};
use orrin_core::{Error, Hash, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::disk::rules::Found;

#[derive(Clone, Serialize, Deserialize)]
struct Stat {
    size: u64,
    modified: Option<SystemTime>,
    state: FileState,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Stats {
    files: HashMap<String, Stat>,
    #[serde(skip)]
    changed: bool,
}

impl Stats {
    pub fn state(&mut self, rel: &str, abs: &Path, meta: &Metadata) -> Result<FileState> {
        if let Some(stat) = self.files.get(rel).filter(|s| matches(s, meta)) {
            return Ok(stat.state.clone());
        }
        let state = FileState { hash: hash_file(abs)?, size: meta.len(), mode: mode_of(meta) };
        self.remember(rel, meta, state.hash.clone());
        Ok(state)
    }

    pub fn states(&mut self, found: &[Found]) -> Result<Vec<FileState>> {
        let hashed: Vec<Option<Result<Hash>>> =
            found.par_iter().map(|f| self.known(&f.rel, &f.meta).is_none().then(|| hash_file(&f.abs))).collect();
        found
            .iter()
            .zip(hashed)
            .map(|(f, hash)| match hash {
                Some(hash) => Ok(self.remember(&f.rel, &f.meta, hash?)),
                None => Ok(self.files[&f.rel].state.clone()),
            })
            .collect()
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
        let stat = Stat { size: meta.len(), modified: meta.modified().ok(), state: state.clone() };
        self.files.insert(rel.to_string(), stat);
        self.changed = true;
        state
    }

    pub fn remember_path(&mut self, rel: &str, abs: &Path, hash: Hash) -> Result<()> {
        let meta = std::fs::metadata(abs)?;
        self.remember(rel, &meta, hash);
        Ok(())
    }

    pub fn known(&self, rel: &str, meta: &Metadata) -> Option<Hash> {
        self.files.get(rel).filter(|s| matches(s, meta)).map(|s| s.state.hash.clone())
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(stat) = self.files.remove(from) {
            self.files.insert(to.to_string(), stat);
            self.changed = true;
        }
    }

    pub fn forget(&mut self, rel: &str) {
        self.changed |= self.files.remove(rel).is_some();
    }

    pub fn forget_under(&mut self, rel: &str) -> Vec<String> {
        let prefix = format!("{rel}/");
        let gone: Vec<String> = self.files.keys().filter(|k| *k == rel || k.starts_with(&prefix)).cloned().collect();
        gone.iter().for_each(|k| self.forget(k));
        gone
    }

    pub fn keep_only(&mut self, seen: &HashSet<String>) {
        let before = self.files.len();
        self.files.retain(|k, _| seen.contains(k));
        self.changed |= self.files.len() != before;
    }

    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
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
