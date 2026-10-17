use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use orrin_core::model::{Snapshot, ViewKind};
use orrin_core::{Error, ProjectId, Result, SnapshotId, ViewId};
use serde::{Deserialize, Serialize};

use crate::disk::files::{remove_if_present, write_fresh};
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::pools;
use crate::snapshots::moved_paths;

pub struct Folder {
    pub id: ViewId,
    pub base: SnapshotId,
    pub root: PathBuf,
    pub stats: Stats,
}

type Gate = Arc<tokio::sync::Mutex<Option<Folder>>>;

#[derive(Default)]
pub struct Spares(Mutex<HashMap<ProjectId, Gate>>);

#[derive(Serialize, Deserialize)]
struct SpareRecord {
    id: ViewId,
    project: ProjectId,
    base: SnapshotId,
}

impl Spares {
    fn gate(&self, project: &ProjectId) -> Gate {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).entry(project.clone()).or_default().clone()
    }
}

impl Inner {
    pub async fn folder_at(&self, project: &ProjectId, base: &SnapshotId) -> Result<Folder> {
        let gate = self.spares.gate(project);
        let spare = gate.lock().await.take();
        if let Some(spare) = &spare {
            remove_if_present(&self.spare_path(&spare.id))?;
        }
        pools::files().install(|| self.prepare(spare, base))
    }

    pub async fn refill(self: Arc<Self>, project: ProjectId) -> Result<()> {
        let gate = self.spares.gate(&project);
        let mut spare = gate.lock_owned().await;
        let main = self.hub.projects.get(&project).await?.main;
        if spare.as_ref().is_some_and(|s| s.base == main) {
            return Ok(());
        }
        let old = spare.take();
        let inner = self.clone();
        let ready = tokio::task::spawn_blocking(move || {
            pools::background().install(|| inner.prepare(old, &main).and_then(|folder| inner.keep(&project, folder)))
        });
        *spare = Some(ready.await.map_err(|e| Error::internal(e.to_string()))??);
        Ok(())
    }

    pub fn adopt_spare(&self, path: &Path, taken: &HashSet<String>) -> Option<ViewId> {
        let record: SpareRecord = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        if taken.contains(record.id.as_str()) {
            return None;
        }
        let gate = self.spares.gate(&record.project);
        let mut spare = gate.try_lock().ok()?;
        if spare.is_some() {
            return None;
        }
        let stats = Stats::load(&self.stats_path(&record.id));
        let root = self.worker_root(&record.id);
        *spare = Some(Folder { id: record.id.clone(), base: record.base, root, stats });
        Some(record.id)
    }

    fn keep(&self, project: &ProjectId, mut folder: Folder) -> Result<Folder> {
        let record = SpareRecord { id: folder.id.clone(), project: project.clone(), base: folder.base.clone() };
        if folder.stats.take_changed() {
            write_fresh(&self.stats_path(&folder.id), &folder.stats.to_bytes()?)?;
        }
        write_fresh(&self.spare_path(&folder.id), &serde_json::to_vec(&record)?)?;
        Ok(folder)
    }

    fn spare_path(&self, id: &ViewId) -> PathBuf {
        self.home.views().join(format!("{id}.spare"))
    }

    fn prepare(&self, spare: Option<Folder>, base: &SnapshotId) -> Result<Folder> {
        let snapshot = self.hub.store.snapshot(base)?;
        if let Some(spare) = spare {
            let root = spare.root.clone();
            match self.bring(spare, &snapshot, base) {
                Ok(folder) => return Ok(folder),
                Err(err) => {
                    tracing::warn!("spare view folder {} could not move to {base}: {err}", root.display());
                    let _ = std::fs::remove_dir_all(&root);
                }
            }
        }
        let id = ViewId::new();
        let root = self.worker_root(&id);
        let stats = self.materialise_all(&snapshot, &root).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&root);
        })?;
        Ok(Folder { id, base: base.clone(), root, stats })
    }

    fn bring(&self, mut folder: Folder, snapshot: &Snapshot, base: &SnapshotId) -> Result<Folder> {
        if folder.base == *base {
            return Ok(folder);
        }
        let old = self.hub.store.snapshot(&folder.base)?;
        for path in moved_paths(&old, snapshot) {
            match snapshot.get(&path) {
                Some(entry) => self.place(ViewKind::Worker, &folder.root, entry, &mut folder.stats)?,
                None => {
                    folder.stats.forget(&path);
                    remove_if_present(&folder.root.join(&path))?;
                }
            }
        }
        folder.base = base.clone();
        Ok(folder)
    }
}
