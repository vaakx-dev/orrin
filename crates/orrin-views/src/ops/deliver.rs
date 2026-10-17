use std::fs::File;
use std::time::{Duration, UNIX_EPOCH};

use orrin_core::model::{Artifact, Delivered, ViewKind, clean_path};
use orrin_core::service::Materialise;
use orrin_core::{ProjectId, Result};

use crate::disk::files::is_locked;
use crate::inner::Inner;
use crate::slot::Slot;

enum Outcome {
    Copied,
    Unchanged,
    Busy,
}

impl Inner {
    pub async fn deliver(&self, project: &ProjectId, files: Vec<Artifact>) -> Result<Delivered> {
        let mut total = Delivered::default();
        let slots = self.slots().into_iter().filter(|s| s.project == *project && s.kind == ViewKind::User);
        for slot in slots {
            let mut record = slot.record.lock().await;
            if !Slot::is_open(&record) {
                continue;
            }
            let known = record.delivered.len();
            record.delivered.extend(files.iter().map(|f| f.path.clone()));
            if record.delivered.len() != known {
                self.save(&record)?;
                self.reload_rules(&slot, &record.delivered).await;
            }
            for file in &files {
                match self.deliver_file(&slot, file)? {
                    Outcome::Copied => total.copied += 1,
                    Outcome::Unchanged => total.unchanged += 1,
                    Outcome::Busy if !total.busy.contains(&file.path) => total.busy.push(file.path.clone()),
                    Outcome::Busy => {}
                }
            }
            self.save_stats(&slot)?;
        }
        Ok(total)
    }

    fn deliver_file(&self, slot: &Slot, file: &Artifact) -> Result<Outcome> {
        let abs = slot.root.join(clean_path(&file.path)?);
        if slot.stats().disk_state(&file.path, &abs)?.is_some_and(|s| s.hash == file.hash) {
            return Ok(Outcome::Unchanged);
        }
        if abs.exists() && is_locked(&abs) {
            return Ok(Outcome::Busy);
        }
        if let Err(err) = self.hub.store.materialise(&file.hash, &abs, Materialise::Copy) {
            return if is_locked(&abs) { Ok(Outcome::Busy) } else { Err(err) };
        }
        if let Some(ms) = file.modified {
            File::options().write(true).open(&abs)?.set_modified(UNIX_EPOCH + Duration::from_millis(ms))?;
        }
        slot.stats().remember_path(&file.path, &abs, file.hash.clone())?;
        Ok(Outcome::Copied)
    }
}
