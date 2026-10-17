use std::collections::HashMap;
use std::sync::MutexGuard;

use orrin_core::model::Snapshot;
use orrin_core::{Result, SnapshotId};

use crate::disk::DiskStore;

const KEEP: usize = 64;

impl DiskStore {
    pub(crate) fn load_snapshot(&self, id: &SnapshotId) -> Result<Snapshot> {
        if let Some(snapshot) = self.cached_snapshots().get(id) {
            return Ok(snapshot.clone());
        }
        let snapshot = Snapshot::from_bytes(&self.verified_bytes(id)?)?;
        self.remember_snapshot(id, &snapshot);
        Ok(snapshot)
    }

    pub(crate) fn save_snapshot(&self, snapshot: &Snapshot) -> Result<SnapshotId> {
        let id = self.put_bytes(&snapshot.to_bytes())?;
        self.remember_snapshot(&id, snapshot);
        Ok(id)
    }

    pub(crate) fn forget_snapshots(&self) {
        self.cached_snapshots().clear();
    }

    fn remember_snapshot(&self, id: &SnapshotId, snapshot: &Snapshot) {
        let mut cached = self.cached_snapshots();
        if cached.len() >= KEEP {
            cached.clear();
        }
        cached.insert(id.clone(), snapshot.clone());
    }

    fn cached_snapshots(&self) -> MutexGuard<'_, HashMap<SnapshotId, Snapshot>> {
        self.snapshots.lock().unwrap_or_else(|e| e.into_inner())
    }
}
