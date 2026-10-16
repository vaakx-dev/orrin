use std::collections::HashSet;

use orrin_core::api::{Hashes, Objects, SyncGet, SyncMissing, SyncPut};
use orrin_core::{Error, Hash, PcId, Result};

use super::{pack, parts_of, unique, unpack};
use crate::service::Inner;

const GET_BATCH: usize = 64;
const CHECK_BATCH: usize = 512;
const PUT_BYTES: usize = 1 << 20;
const MAX_DEPTH: usize = 8;

impl Inner {
    pub async fn pull(&self, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
        let mut wanted = unique(hashes);
        for _ in 0..MAX_DEPTH {
            if wanted.is_empty() {
                break;
            }
            let missing = self.hub.store.missing(&wanted);
            self.fetch(pc, missing).await?;
            wanted = parts_of(self.hub.store.as_ref(), &wanted);
        }
        Ok(())
    }

    pub async fn push(&self, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
        let mut wanted = unique(hashes);
        for _ in 0..MAX_DEPTH {
            if wanted.is_empty() {
                break;
            }
            for batch in wanted.chunks(CHECK_BATCH) {
                let lacking = self.ask(pc, SyncMissing(Hashes { hashes: batch.to_vec() })).await?.hashes;
                self.send(pc, lacking).await?;
            }
            wanted = parts_of(self.hub.store.as_ref(), &wanted);
        }
        Ok(())
    }

    async fn fetch(&self, pc: &PcId, mut remaining: Vec<Hash>) -> Result<()> {
        while !remaining.is_empty() {
            let batch = remaining.iter().take(GET_BATCH).cloned().collect();
            let objects = self.ask(pc, SyncGet(Hashes { hashes: batch })).await?.objects;
            let mut received = HashSet::new();
            for object in &objects {
                self.hub.store.put_raw(&object.hash, &unpack(object)?)?;
                received.insert(object.hash.clone());
            }
            let before = remaining.len();
            remaining.retain(|h| !received.contains(h));
            if remaining.len() == before {
                return Err(Error::not_found(format!("{pc} does not have {}", remaining[0])));
            }
        }
        Ok(())
    }

    async fn send(&self, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
        let mut batch = Vec::new();
        let mut bytes = 0;
        for hash in hashes {
            let raw = self.hub.store.raw(&hash)?;
            if !batch.is_empty() && bytes + raw.len() > PUT_BYTES {
                self.ask(pc, SyncPut(Objects { objects: std::mem::take(&mut batch) })).await?;
                bytes = 0;
            }
            bytes += raw.len();
            batch.push(pack(hash, &raw));
        }
        if !batch.is_empty() {
            self.ask(pc, SyncPut(Objects { objects: batch })).await?;
        }
        Ok(())
    }
}
