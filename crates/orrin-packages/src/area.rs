use std::path::{Path, PathBuf};

use orrin_core::id::fresh;
use orrin_core::service::Store;
use orrin_core::{Hash, Millis, Result, SnapshotId, now_ms};
use serde::{Deserialize, Serialize};

use crate::folder;

const DAY_MS: Millis = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy)]
pub enum Kind {
    Source,
    Built,
}

#[derive(Serialize, Deserialize)]
struct Record {
    name: String,
    snapshot: SnapshotId,
    used_at: Millis,
}

pub struct Area {
    root: PathBuf,
}

impl Area {
    pub fn open(root: PathBuf) -> Result<Self> {
        for kind in [Kind::Source, Kind::Built] {
            std::fs::create_dir_all(root.join(folder_of(kind)))?;
        }
        std::fs::create_dir_all(root.join("fetch"))?;
        Ok(Self { root })
    }

    pub fn get(&self, store: &dyn Store, kind: Kind, key: &Hash) -> Option<SnapshotId> {
        let bytes = std::fs::read(self.record(kind, key)).ok()?;
        let record: Record = serde_json::from_slice(&bytes).ok()?;
        let snapshot = store.snapshot(&record.snapshot).ok()?;
        let hashes: Vec<Hash> = snapshot.entries.into_iter().map(|e| e.hash).collect();
        store.missing(&hashes).is_empty().then_some(record.snapshot)
    }

    pub fn put(&self, kind: Kind, key: &Hash, name: &str, snapshot: &SnapshotId) -> Result<()> {
        let record = Record { name: name.to_string(), snapshot: snapshot.clone(), used_at: now_ms() };
        let path = self.record(kind, key);
        let temp = path.with_extension(fresh("tmp"));
        std::fs::write(&temp, serde_json::to_vec(&record)?)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }

    pub fn fetch_dir(&self, key: &Hash) -> PathBuf {
        self.root.join("fetch").join(format!("{}-{}", short(key), fresh("f")))
    }

    pub fn place(&self, store: &dyn Store, key: &Hash, snapshot: &SnapshotId) -> Result<PathBuf> {
        let dir = self.built_dir(key);
        if dir.is_dir() {
            return Ok(dir);
        }
        let temp = self.root.join(folder_of(Kind::Built)).join(format!(".{}-{}", short(key), fresh("t")));
        let filled = folder::fill(store, &temp, &store.snapshot(snapshot)?);
        match filled.and_then(|()| Ok(std::fs::rename(&temp, &dir)?)) {
            Ok(()) => Ok(dir),
            Err(_) if dir.is_dir() => {
                remove(&temp);
                Ok(dir)
            }
            Err(err) => {
                remove(&temp);
                Err(err)
            }
        }
    }

    pub fn clean(&self, keep_days: u32) -> u32 {
        let cutoff = now_ms().saturating_sub(keep_days as Millis * DAY_MS);
        let mut removed = 0;
        for (kind, key, record) in self.records() {
            if record.used_at < cutoff {
                let _ = std::fs::remove_file(self.record(kind, &key));
                if matches!(kind, Kind::Built) {
                    remove(&self.built_dir(&key));
                }
                removed += 1;
            }
        }
        removed
    }

    pub fn roots(&self) -> Vec<Hash> {
        self.records().into_iter().map(|(_, _, record)| record.snapshot).collect()
    }

    fn records(&self) -> Vec<(Kind, Hash, Record)> {
        let mut found = Vec::new();
        for kind in [Kind::Source, Kind::Built] {
            for entry in std::fs::read_dir(self.root.join(folder_of(kind))).into_iter().flatten().flatten() {
                let path = entry.path();
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
                let (Ok(key), Some("json")) = (Hash::parse(stem), path.extension().and_then(|e| e.to_str())) else {
                    continue;
                };
                let Ok(bytes) = std::fs::read(&path) else { continue };
                if let Ok(record) = serde_json::from_slice(&bytes) {
                    found.push((kind, key, record));
                }
            }
        }
        found
    }

    fn record(&self, kind: Kind, key: &Hash) -> PathBuf {
        self.root.join(folder_of(kind)).join(format!("{key}.json"))
    }

    fn built_dir(&self, key: &Hash) -> PathBuf {
        self.root.join(folder_of(Kind::Built)).join(key.as_str())
    }
}

fn folder_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Source => "sources",
        Kind::Built => "built",
    }
}

fn short(key: &Hash) -> &str {
    &key.as_str()[..16]
}

pub fn remove(dir: &Path) {
    if dir.exists()
        && let Err(err) = std::fs::remove_dir_all(dir)
    {
        tracing::warn!("could not remove package folder {}: {err}", dir.display());
    }
}
