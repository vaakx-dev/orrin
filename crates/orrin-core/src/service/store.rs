use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::id::{Hash, SnapshotId};
use crate::model::Snapshot;
use crate::service::Service;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Materialise {
    Copy,
    ReadonlyCopy,
    Link,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcReport {
    pub removed: u64,
    pub freed: u64,
}

pub trait Store: Service {
    fn put(&self, bytes: &[u8]) -> Result<Hash>;
    fn put_file(&self, path: &Path) -> Result<Hash>;
    fn get(&self, hash: &Hash) -> Result<Vec<u8>>;
    fn size(&self, hash: &Hash) -> Result<u64>;
    fn has(&self, hash: &Hash) -> bool;
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash>;
    fn materialise(&self, hash: &Hash, path: &Path, how: Materialise) -> Result<()>;
    fn gc(&self, roots: &[Hash]) -> Result<GcReport>;
    fn parts(&self, hash: &Hash) -> Result<Vec<Hash>>;
    fn raw(&self, hash: &Hash) -> Result<Vec<u8>>;
    fn put_raw(&self, hash: &Hash, bytes: &[u8]) -> Result<()>;

    fn put_snapshot(&self, snapshot: &Snapshot) -> Result<SnapshotId> {
        self.put(&snapshot.to_bytes())
    }

    fn snapshot(&self, id: &SnapshotId) -> Result<Snapshot> {
        Snapshot::from_bytes(&self.get(id)?)
    }
}
