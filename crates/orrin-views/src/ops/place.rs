use std::fs::Metadata;
use std::path::Path;

use orrin_core::model::{Entry, Mode, ViewKind};
use orrin_core::service::Materialise;
use orrin_core::{Hash, Result};

use crate::disk::files::{is_sealed, remove_if_present, set_executable};
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::slot::Slot;

impl Inner {
    pub fn place(&self, kind: ViewKind, root: &Path, entry: &Entry, stats: &mut Stats) -> Result<()> {
        let meta = self.write_entry(kind, root, entry)?;
        stats.remember(&entry.path, &meta, entry.hash.clone());
        Ok(())
    }

    pub fn write_entry(&self, kind: ViewKind, root: &Path, entry: &Entry) -> Result<Metadata> {
        let abs = root.join(&entry.path);
        remove_if_present(&abs)?;
        let how = match (kind, entry.mode) {
            (ViewKind::Worker, Mode::File) => Materialise::Link,
            _ => Materialise::Copy,
        };
        self.hub.store.materialise(&entry.hash, &abs, how)?;
        if entry.mode == Mode::Executable {
            set_executable(&abs)?;
        }
        Ok(std::fs::metadata(&abs)?)
    }

    pub fn own(&self, slot: &Slot, rel: &str, hash: Hash) -> Result<()> {
        let abs = slot.root.join(rel);
        if !is_sealed(&abs) {
            return Ok(());
        }
        self.hub.store.materialise(&hash, &abs, Materialise::Copy)?;
        slot.stats().remember_path(rel, &abs, hash)
    }
}
