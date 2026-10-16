use std::path::Path;

use orrin_core::service::{GcReport, Materialise, Service, Store};
use orrin_core::{Hash, Result};

use crate::disk::DiskStore;
use crate::verified::Verified;

impl Service for DiskStore {}

impl Store for DiskStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        self.put_bytes(bytes)
    }

    fn put_file(&self, path: &Path) -> Result<Hash> {
        self.put_path(path)
    }

    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        let mut out = Verified::new(Vec::new());
        self.write_to(hash, &mut out)?;
        out.finish(hash)
    }

    fn size(&self, hash: &Hash) -> Result<u64> {
        self.object_size(hash)
    }

    fn has(&self, hash: &Hash) -> bool {
        self.exists(hash)
    }

    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        hashes.iter().filter(|hash| !self.exists(hash)).cloned().collect()
    }

    fn materialise(&self, hash: &Hash, path: &Path, how: Materialise) -> Result<()> {
        self.materialise_at(hash, path, how)
    }

    fn gc(&self, roots: &[Hash]) -> Result<GcReport> {
        self.collect(roots)
    }

    fn parts(&self, hash: &Hash) -> Result<Vec<Hash>> {
        self.closure(hash)
    }

    fn raw(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.raw_bytes(hash)
    }

    fn put_raw(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        self.put_raw_object(hash, bytes)
    }
}
