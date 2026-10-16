mod serve;
mod transfer;

use std::collections::HashSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::Object;
use orrin_core::model::Snapshot;
use orrin_core::service::Store;
use orrin_core::{Error, Hash, Result, SnapshotId};

pub fn pack(hash: Hash, raw: &[u8]) -> Object {
    Object { hash, data: STANDARD.encode(raw) }
}

pub fn unpack(object: &Object) -> Result<Vec<u8>> {
    STANDARD.decode(&object.data).map_err(|_| Error::invalid(format!("object {} is not base64", object.hash)))
}

pub fn delta(store: &dyn Store, from: Option<&SnapshotId>, to: &Snapshot) -> Vec<Hash> {
    let base: HashSet<Hash> = from
        .and_then(|id| store.snapshot(id).ok())
        .map(|s| s.entries.into_iter().map(|e| e.hash).collect())
        .unwrap_or_default();
    unique(to.entries.iter().map(|e| e.hash.clone()).filter(|h| !base.contains(h)))
}

pub fn unique(hashes: impl IntoIterator<Item = Hash>) -> Vec<Hash> {
    let mut seen = HashSet::new();
    hashes.into_iter().filter(|h| seen.insert(h.clone())).collect()
}

fn parts_of(store: &dyn Store, hashes: &[Hash]) -> Vec<Hash> {
    unique(hashes.iter().flat_map(|h| store.parts(h).unwrap_or_default()))
}
