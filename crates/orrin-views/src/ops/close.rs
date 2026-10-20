use orrin_core::api::{Done, ViewDrop, ViewList, ViewListOutput};
use orrin_core::model::{View, ViewKind, ViewState};
use orrin_core::{Result, ViewId, now_ms};

use crate::disk::files::{remove_any, retired};
use crate::inner::Inner;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

impl Inner {
    pub async fn get(&self, id: &ViewId) -> Result<View> {
        Ok(self.slot(id)?.record.lock().await.view.clone())
    }

    pub async fn list(&self, input: ViewList) -> Result<ViewListOutput> {
        let mut views = Vec::new();
        for slot in self.slots() {
            let view = slot.record.lock().await.view.clone();
            let wanted = input.project.as_ref().is_none_or(|p| *p == view.project);
            if wanted && view.state != ViewState::Dropped {
                views.push(view);
            }
        }
        views.sort_by_key(|v| v.created_at);
        Ok(ViewListOutput { views })
    }

    pub async fn remove(&self, input: ViewDrop) -> Result<Done> {
        let slot = self.slot(&input.view)?;
        let mut record = slot.record.lock().await;
        slot.set_watcher(None);
        record.view.state = ViewState::Dropped;
        record.closed_at.get_or_insert_with(now_ms);
        self.save(&record)?;
        Ok(Done {})
    }

    pub async fn purge(&self) -> Result<u32> {
        let cutoff = now_ms().saturating_sub(u64::from(self.config.keep_days) * DAY_MS);
        let mut purged = 0;
        for slot in self.slots() {
            let closed = slot.record.lock().await.closed_at;
            if closed.is_none_or(|at| at > cutoff) {
                continue;
            }
            if slot.kind == ViewKind::Worker {
                if slot.root.exists() {
                    std::fs::remove_dir_all(&slot.root)?;
                }
                remove_any(&retired(&slot.root));
            }
            self.forget(&slot.id)?;
            purged += 1;
        }
        Ok(purged)
    }
}
