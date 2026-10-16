use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::{Hashes, LandOutput, Object, Objects, Request, SyncGet, SyncMissing, SyncPut};
use orrin_core::service::{Services, call_peer};
use orrin_core::{Error, Hash, PcId, Result};

pub async fn forward<R: Request<Output = LandOutput>>(hub: &Services, home: &PcId, input: R) -> Result<LandOutput> {
    let out = call_peer::<R>(&*hub.peers, home, input).await?;
    if let Err(err) = adopt(hub, home, &out).await {
        tracing::warn!(pc = %home, "landed on the home PC but could not fetch the result: {err}");
    }
    Ok(out)
}

pub async fn push(hub: &Services, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
    let mut want = hashes;
    while !want.is_empty() {
        let missing = call_peer(&*hub.peers, pc, SyncMissing(Hashes { hashes: want })).await?.hashes;
        if missing.is_empty() {
            break;
        }
        let objects = missing
            .iter()
            .map(|hash| Ok(Object { hash: hash.clone(), data: STANDARD.encode(hub.store.raw(hash)?) }))
            .collect::<Result<Vec<_>>>()?;
        call_peer(&*hub.peers, pc, SyncPut(Objects { objects })).await?;
        want = parts(hub, &missing);
    }
    Ok(())
}

async fn adopt(hub: &Services, home: &PcId, out: &LandOutput) -> Result<()> {
    let theirs = out.conflicts.iter().flat_map(|c| c.theirs.iter().chain(c.base.iter()).cloned()).collect();
    pull(hub, home, theirs).await?;
    let Some(change) = &out.change else {
        return Ok(());
    };
    pull(hub, home, vec![out.main.clone()]).await?;
    let files = hub.store.snapshot(&out.main)?.entries.into_iter().map(|e| e.hash).collect();
    pull(hub, home, files).await?;
    let mut project = hub.projects.get(&change.project).await?;
    project.main = out.main.clone();
    hub.projects.save(project).await?;
    hub.projects.append(vec![change.clone()]).await
}

async fn pull(hub: &Services, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
    let mut want = hub.store.missing(&hashes);
    while !want.is_empty() {
        let objects = call_peer(&*hub.peers, pc, SyncGet(Hashes { hashes: want })).await?.objects;
        let mut fetched = Vec::new();
        for object in objects {
            let bytes = STANDARD.decode(&object.data).map_err(|e| Error::invalid(format!("bad object data: {e}")))?;
            hub.store.put_raw(&object.hash, &bytes)?;
            fetched.push(object.hash);
        }
        if fetched.is_empty() {
            break;
        }
        want = hub.store.missing(&parts(hub, &fetched));
    }
    Ok(())
}

fn parts(hub: &Services, hashes: &[Hash]) -> Vec<Hash> {
    hashes.iter().flat_map(|hash| hub.store.parts(hash).unwrap_or_default()).collect()
}
