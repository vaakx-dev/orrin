use orrin_core::api::Event;
use orrin_core::model::{Entry, ViewKind, ViewState};
use orrin_core::service::Edits;
use orrin_core::{ChangeId, Error, Result, SnapshotId, ViewId, now_ms};

use crate::disk::files::{is_locked, remove_if_present};
use crate::inner::Inner;
use crate::slot::Slot;
use crate::snapshots::{keeping, moved_paths, with_edits};

impl Inner {
    pub async fn advance(&self, id: &ViewId, snapshot: &SnapshotId) -> Result<Vec<String>> {
        let slot = self.slot(id)?;
        let mut record = slot.record.lock().await;
        let old = self.hub.store.snapshot(&record.view.base)?;
        let new = self.hub.store.snapshot(snapshot)?;
        let mut skipped = Vec::new();
        let mut written = Vec::new();
        for path in moved_paths(&old, &new) {
            let wanted = new.get(&path);
            let current = slot.stats().disk_state(&path, &slot.root.join(&path))?.map(|s| s.hash);
            if current.as_ref() == wanted.map(|e| &e.hash) {
                continue;
            }
            let untouched = current.as_ref() == old.get(&path).map(|e| &e.hash);
            if untouched && self.update(&slot, &path, wanted).is_ok() {
                written.push(path);
            } else {
                skipped.push(path);
            }
        }
        let base = keeping(&new, &old, &skipped);
        record.view.base = if skipped.is_empty() { snapshot.clone() } else { self.hub.store.put_snapshot(&base)? };
        record.captured.clear();
        self.save(&record)?;
        drop(record);
        if !written.is_empty() {
            self.touched(&slot, written).await;
        }
        if !skipped.is_empty() {
            self.events.emit(Event::ViewSkipped { view: id.clone(), paths: skipped.clone() });
        }
        Ok(skipped)
    }

    pub async fn take(&self, id: &ViewId, expected: &SnapshotId, files: &Edits) -> Result<Vec<String>> {
        let slot = self.slot(id)?;
        let record = slot.record.lock().await;
        let expected = self.hub.store.snapshot(expected)?;
        let mut missed = Vec::new();
        let mut written = Vec::new();
        for (path, state) in files {
            let current = slot.stats().disk_state(path, &slot.root.join(path))?.map(|s| s.hash);
            let wanted =
                state.as_ref().map(|s| Entry { path: path.clone(), hash: s.hash.clone(), size: s.size, mode: s.mode });
            let unchanged = current.as_ref() == expected.get(path).map(|e| &e.hash);
            if unchanged && self.update(&slot, path, wanted.as_ref()).is_ok() {
                written.push(path.clone());
            } else {
                missed.push(path.clone());
            }
        }
        drop(record);
        if !written.is_empty() {
            self.touched(&slot, written).await;
        }
        Ok(missed)
    }

    pub async fn landed(&self, id: &ViewId, change: Option<&ChangeId>) -> Result<()> {
        let slot = self.slot(id)?;
        let change = match change {
            Some(change) => Some(self.hub.projects.change(change).await?),
            None => None,
        };
        let mut record = slot.record.lock().await;
        record.view.landed = change.as_ref().map(|c| c.id.clone());
        match (slot.kind, change) {
            (ViewKind::Worker, _) => {
                record.view.state = ViewState::Landed;
                record.closed_at = Some(now_ms());
                slot.set_watcher(None);
            }
            (ViewKind::User, None) => return Err(Error::invalid(format!("user view {id} can only land to main"))),
            (ViewKind::User, Some(change)) => {
                let ours: Edits = change
                    .files
                    .iter()
                    .map(|(path, state)| {
                        (path.clone(), record.captured.get(path).cloned().unwrap_or_else(|| state.clone()))
                    })
                    .collect();
                let base = self.hub.store.snapshot(&record.view.base)?;
                record.view.base = self.hub.store.put_snapshot(&with_edits(&base, &ours))?;
                record.captured.clear();
            }
        }
        self.save(&record)
    }

    fn update(&self, slot: &Slot, path: &str, wanted: Option<&Entry>) -> Result<()> {
        let abs = slot.root.join(path);
        if abs.exists() && is_locked(&abs) {
            return Err(Error::conflict(format!("{path} is open")));
        }
        let mut stats = slot.stats();
        match wanted {
            Some(entry) => self.place(slot.kind, &slot.root, entry, &mut stats),
            None => {
                stats.forget(path);
                remove_if_present(&abs)
            }
        }
    }
}
