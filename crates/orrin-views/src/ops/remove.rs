use std::path::Path;

use orrin_core::model::ViewKind;
use orrin_core::{ProjectId, Result};

use crate::disk::files::{remove_any, remove_if_present, retired};
use crate::inner::Inner;

impl Inner {
    pub async fn remove_project(&self, project: &ProjectId) -> Result<u32> {
        self.drop_spare(project).await?;
        let slots: Vec<_> = self.slots().into_iter().filter(|s| s.project == *project).collect();
        for slot in &slots {
            let _record = slot.record.lock().await;
            slot.set_watcher(None);
            if slot.kind == ViewKind::Worker {
                remove_dir(&slot.root)?;
                remove_any(&retired(&slot.root));
            }
            self.forget(&slot.id)?;
        }
        Ok(slots.len() as u32)
    }

    async fn drop_spare(&self, project: &ProjectId) -> Result<()> {
        let Some(folder) = self.spares.remove(project).await else { return Ok(()) };
        remove_dir(&folder.root)?;
        remove_if_present(&self.spare_path(&folder.id))?;
        remove_if_present(&self.stats_path(&folder.id))
    }
}

fn remove_dir(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
        _ => Ok(()),
    }
}
