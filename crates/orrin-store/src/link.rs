use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use orrin_core::{Hash, Result};

use crate::disk::{DiskStore, place_new};
use crate::materialise::place;
use crate::object::Manifest;
use crate::seal::{is_sealed, seal, unseal};

const UNKNOWN: u8 = 0;
const CLONES: u8 = 1;
const LINKS: u8 = 2;

impl DiskStore {
    pub(crate) fn link_to(&self, hash: &Hash, path: &Path) -> Result<()> {
        let source = self.link_source(hash)?;
        if self.clone_to(&source, path) {
            return Ok(());
        }
        self.ensure_sealed(&source)?;
        match place(path, |target| fs::hard_link(&source, target)) {
            Ok(()) => Ok(()),
            Err(_) => self.copy_to(hash, path, false),
        }
    }

    fn link_source(&self, hash: &Hash) -> Result<PathBuf> {
        match self.manifest(hash)? {
            None => Ok(self.object_path(hash)),
            Some(Manifest::Chunks { chunks, .. }) => self.full_file(hash, &chunks),
            Some(_) => self.derived_file(hash),
        }
    }

    fn derived_file(&self, hash: &Hash) -> Result<PathBuf> {
        let path = self.file_path(hash);
        if !path.is_file() {
            let temp = self.temp_path();
            self.write_file(hash, &temp, false)?;
            seal(&temp)?;
            place_new(&temp, &path)?;
        }
        Ok(path)
    }

    fn clone_to(&self, source: &Path, path: &Path) -> bool {
        if self.clones.load(Ordering::Relaxed) == LINKS {
            return false;
        }
        let cloned = place(path, |target| reflink_copy::reflink(source, target)).is_ok() && unseal(path).is_ok();
        let known = if cloned { CLONES } else { LINKS };
        let _ = self.clones.compare_exchange(UNKNOWN, known, Ordering::Relaxed, Ordering::Relaxed);
        cloned
    }

    fn ensure_sealed(&self, source: &Path) -> Result<()> {
        if self.sealed.lock().unwrap_or_else(|e| e.into_inner()).contains(source) {
            return Ok(());
        }
        if !is_sealed(source) {
            seal(source)?;
        }
        self.sealed.lock().unwrap_or_else(|e| e.into_inner()).insert(source.to_path_buf());
        Ok(())
    }
}
