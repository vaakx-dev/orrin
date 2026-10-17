use std::collections::{BTreeSet, HashSet};

use orrin_core::api::{ChangedFile, Changes, ChangesOutput, Diff, DiffOutput};
use orrin_core::model::{Entry, FileState, FileStatus, Snapshot};
use orrin_core::service::Edits;
use orrin_core::{Error, Hash, Result, ViewId};

use crate::content::diff::{file_diff, under};
use crate::disk::rules::Rules;
use crate::inner::Inner;
use crate::slot::Slot;

impl Inner {
    pub async fn capture(&self, id: &ViewId) -> Result<Edits> {
        let slot = self.slot(id)?;
        let rules = self.refresh_rules(&slot).await;
        let mut record = slot.record.lock().await;
        let base = self.hub.store.snapshot(&record.view.base)?;
        let edits = self.scan(&slot, &rules, &base, &record.delivered)?;
        if record.captured != edits {
            record.captured = edits.clone();
            self.save(&record)?;
        }
        self.save_stats(&slot)?;
        Ok(edits)
    }

    fn scan(&self, slot: &Slot, rules: &Rules, base: &Snapshot, delivered: &BTreeSet<String>) -> Result<Edits> {
        let mut edits = Edits::new();
        let mut seen: HashSet<String> = delivered.iter().cloned().collect();
        let found = rules.walk();
        let mut stats = slot.stats();
        for (found, state) in found.iter().zip(stats.states(&found)?) {
            if !same_entry(base, &found.rel, &state) {
                edits.insert(found.rel.clone(), Some(self.stored(state, &found.abs)?));
            }
            seen.insert(found.rel.clone());
        }
        let unseen: Vec<&Entry> = base.entries.iter().filter(|e| !seen.contains(&e.path)).collect();
        for entry in unseen {
            let abs = slot.root.join(&entry.path);
            let state = match stats.disk_state(&entry.path, &abs)? {
                Some(state) if state.hash == entry.hash => continue,
                Some(state) => Some(self.stored(state, &abs)?),
                None => None,
            };
            edits.insert(entry.path.clone(), state);
        }
        seen.extend(base.entries.iter().map(|e| e.path.clone()));
        stats.keep_only(&seen);
        Ok(edits)
    }

    pub async fn changes(&self, input: Changes) -> Result<ChangesOutput> {
        let slot = self.slot_in(&input.project, &input.view)?;
        let edits = self.capture(&input.view).await?;
        let base_id = slot.record.lock().await.view.base.clone();
        let base = self.hub.store.snapshot(&base_id)?;
        let files = edits
            .iter()
            .map(|(path, state)| ChangedFile { path: path.clone(), status: status(&base, path, state) })
            .collect();
        Ok(ChangesOutput { base: base_id, files })
    }

    pub async fn diff(&self, input: Diff) -> Result<DiffOutput> {
        let (parent, files) = match (&input.view, &input.change) {
            (Some(view), _) => {
                let slot = self.slot_in(&input.project, view)?;
                let edits = self.capture(view).await?;
                (slot.record.lock().await.view.base.clone(), edits)
            }
            (None, Some(change)) => {
                let change = self.hub.projects.change(change).await?;
                (change.parent, change.files)
            }
            (None, None) => return Err(Error::invalid("diff needs a view or a change")),
        };
        let base = self.hub.store.snapshot(&parent)?;
        let files = files
            .iter()
            .filter(|(path, _)| under(path, input.path.as_deref()))
            .map(|(path, state)| {
                let old = self.bytes(base.get(path).map(|e| &e.hash))?;
                let new = self.bytes(state.as_ref().map(|s| &s.hash))?;
                Ok(file_diff(path, status(&base, path, state), &old, &new))
            })
            .collect::<Result<_>>()?;
        Ok(DiffOutput { files })
    }

    fn stored(&self, mut state: FileState, abs: &std::path::Path) -> Result<FileState> {
        if !self.hub.store.has(&state.hash) {
            state.hash = self.hub.store.put_file(abs)?;
        }
        Ok(state)
    }

    fn bytes(&self, hash: Option<&Hash>) -> Result<Vec<u8>> {
        hash.map_or(Ok(Vec::new()), |h| self.hub.store.get(h))
    }
}

fn same_entry(base: &Snapshot, path: &str, state: &FileState) -> bool {
    base.get(path).is_some_and(|e| e.hash == state.hash && e.mode == state.mode)
}

fn status(base: &Snapshot, path: &str, state: &Option<FileState>) -> FileStatus {
    match (state, base.get(path)) {
        (None, _) => FileStatus::Deleted,
        (Some(_), Some(_)) => FileStatus::Modified,
        (Some(_), None) => FileStatus::Added,
    }
}
