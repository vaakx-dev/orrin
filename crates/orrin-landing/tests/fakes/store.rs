use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use orrin_core::service::{GcReport, Materialise, Service, Store};
use orrin_core::{Error, Hash, Result};

#[derive(Default)]
pub struct FakeStore(Mutex<HashMap<Hash, Vec<u8>>>);

impl Service for FakeStore {}

impl Store for FakeStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(hash)
    }

    fn put_file(&self, path: &Path) -> Result<Hash> {
        self.put(&std::fs::read(path)?)
    }

    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.0.lock().unwrap().get(hash).cloned().ok_or_else(|| Error::not_found(hash.to_string()))
    }

    fn size(&self, hash: &Hash) -> Result<u64> {
        Ok(self.get(hash)?.len() as u64)
    }

    fn has(&self, hash: &Hash) -> bool {
        self.0.lock().unwrap().contains_key(hash)
    }

    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        hashes.iter().filter(|h| !self.has(h)).cloned().collect()
    }

    fn materialise(&self, _: &Hash, _: &Path, _: Materialise) -> Result<()> {
        Err(Error::unavailable("fake"))
    }

    fn gc(&self, _: &[Hash]) -> Result<GcReport> {
        Ok(GcReport::default())
    }

    fn parts(&self, _: &Hash) -> Result<Vec<Hash>> {
        Ok(Vec::new())
    }

    fn raw(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.get(hash)
    }

    fn put_raw(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(())
    }
}
