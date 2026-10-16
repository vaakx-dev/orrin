use std::fs::Metadata;
use std::path::Path;

use orrin_core::Result;
use orrin_core::model::{Entry, Mode, ViewKind};
use orrin_core::service::Materialise;

use crate::disk::files::{remove_if_present, set_executable};
use crate::disk::stats::Stats;
use crate::inner::Inner;

impl Inner {
    pub fn place(&self, kind: ViewKind, root: &Path, entry: &Entry, stats: &mut Stats) -> Result<()> {
        let meta = self.write_entry(kind, root, entry)?;
        stats.remember(&entry.path, &meta, entry.hash.clone());
        Ok(())
    }

    pub fn write_entry(&self, kind: ViewKind, root: &Path, entry: &Entry) -> Result<Metadata> {
        let abs = root.join(&entry.path);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        remove_if_present(&abs)?;
        let how = if kind == ViewKind::Worker && entry.size > self.config.big_file {
            Materialise::Link
        } else {
            Materialise::Copy
        };
        self.hub.store.materialise(&entry.hash, &abs, how)?;
        if entry.mode == Mode::Executable && how == Materialise::Copy {
            set_executable(&abs)?;
        }
        Ok(std::fs::metadata(&abs)?)
    }
}
