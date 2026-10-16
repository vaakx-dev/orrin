use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::{Hashes, Object, Objects, SyncGet, SyncMissing, SyncPut};
use orrin_core::service::{Services, call_peer};
use orrin_core::{Error, Hash, PcId, Result};

const BATCH_BYTES: usize = 8 * 1024 * 1024;

pub async fn push(hub: &Services, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
    let mut want = hashes;
    while !want.is_empty() {
        let missing = call_peer(&*hub.peers, pc, SyncMissing(Hashes { hashes: want })).await?.hashes;
        send(hub, pc, &missing).await?;
        want = missing.iter().flat_map(|h| hub.store.parts(h).unwrap_or_default()).collect();
    }
    Ok(())
}

pub async fn pull(hub: &Services, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
    let mut want = hub.store.missing(&hashes);
    while !want.is_empty() {
        let objects = call_peer(&*hub.peers, pc, SyncGet(Hashes { hashes: want.clone() })).await?.objects;
        if objects.is_empty() {
            return Err(Error::not_found(format!("{pc} could not send {} objects", want.len())));
        }
        for object in &objects {
            let bytes = STANDARD.decode(&object.data).map_err(|err| Error::invalid(err.to_string()))?;
            hub.store.put_raw(&object.hash, &bytes)?;
        }
        let parts: Vec<Hash> = objects.iter().flat_map(|o| hub.store.parts(&o.hash).unwrap_or_default()).collect();
        want = hub.store.missing(&parts);
    }
    Ok(())
}

async fn send(hub: &Services, pc: &PcId, hashes: &[Hash]) -> Result<()> {
    let mut batch = Vec::new();
    let mut size = 0;
    for hash in hashes {
        let bytes = hub.store.raw(hash)?;
        size += bytes.len();
        batch.push(Object { hash: hash.clone(), data: STANDARD.encode(bytes) });
        if size >= BATCH_BYTES {
            call_peer(&*hub.peers, pc, SyncPut(Objects { objects: std::mem::take(&mut batch) })).await?;
            size = 0;
        }
    }
    if !batch.is_empty() {
        call_peer(&*hub.peers, pc, SyncPut(Objects { objects: batch })).await?;
    }
    Ok(())
}
