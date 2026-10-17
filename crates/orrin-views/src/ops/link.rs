use orrin_core::model::{Linked, Snapshot, ViewKind, clean_path};
use orrin_core::service::Materialise;
use orrin_core::{ProjectId, Result, ViewId};
use rayon::prelude::*;

use crate::disk::files::remove_if_present;
use crate::inner::Inner;
use crate::pools;
use crate::slot::Slot;
use crate::snapshots::moved_paths;

impl Inner {
    pub async fn link(&self, project: &ProjectId, linked: Linked) -> Result<()> {
        let files = self.hub.store.snapshot(&linked.files)?;
        let slots = self.slots().into_iter().filter(|s| s.project == *project && s.kind == ViewKind::Worker);
        for slot in slots {
            if let Err(err) = self.link_into(&slot, &linked, &files).await {
                tracing::warn!("could not link {} outputs into view {}: {err}", linked.target, slot.id);
            }
        }
        Ok(())
    }

    pub async fn link_latest(&self, id: &ViewId) {
        let Ok(slot) = self.slot(id) else { return };
        let latest = match self.hub.builds.linked(&slot.project).await {
            Ok(latest) => latest,
            Err(err) => return tracing::warn!("could not ask builds for linked outputs: {err}"),
        };
        for linked in latest {
            let linking = match self.hub.store.snapshot(&linked.files) {
                Ok(files) => self.link_into(&slot, &linked, &files).await,
                Err(err) => Err(err),
            };
            if let Err(err) = linking {
                tracing::warn!("could not link {} outputs into view {id}: {err}", linked.target);
            }
        }
    }

    async fn link_into(&self, slot: &Slot, linked: &Linked, files: &Snapshot) -> Result<()> {
        let mut record = slot.record.lock().await;
        if !Slot::is_open(&record) || record.linked.get(&linked.target) == Some(&linked.files) {
            return Ok(());
        }
        let old = record.linked.get(&linked.target).and_then(|id| self.hub.store.snapshot(id).ok());
        let paths: Vec<String> = moved_paths(&old.unwrap_or_default(), files).into_iter().collect();
        pools::files().install(|| paths.par_iter().try_for_each(|path| self.relink(slot, files, path)))?;
        record.linked.insert(linked.target.clone(), linked.files.clone());
        self.save(&record)
    }

    fn relink(&self, slot: &Slot, files: &Snapshot, path: &str) -> Result<()> {
        let abs = slot.root.join(clean_path(path)?);
        remove_if_present(&abs)?;
        match files.get(path) {
            Some(entry) => self.hub.store.materialise(&entry.hash, &abs, Materialise::Link),
            None => Ok(()),
        }
    }
}
