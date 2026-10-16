use std::path::Path;

use orrin_core::Result;
use orrin_core::model::{Entry, Mode, ViewKind};
use orrin_core::service::Materialise;

use crate::disk::files::{remove_if_present, set_executable};
use crate::disk::stats::Stats;
use crate::inner::Inner;

impl Inner {
    pub fn place(&self, kind: ViewKind, root: &Path, entry: &Entry, stats: &mut Stats) -> Result<()> {
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
        stats.remember_path(&entry.path, &abs, entry.hash.clone())
    }
}
