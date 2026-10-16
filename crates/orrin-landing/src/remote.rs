use orrin_core::api::{LandOutput, Request};
use orrin_core::service::{Services, call_peer};
use orrin_core::{PcId, Result};

pub async fn forward<R: Request<Output = LandOutput>>(hub: &Services, home: &PcId, input: R) -> Result<LandOutput> {
    let out = call_peer::<R>(&*hub.peers, home, input).await?;
    if let Err(err) = adopt(hub, home, &out).await {
        tracing::warn!(pc = %home, "landed on the home PC but could not fetch the result: {err}");
    }
    Ok(out)
}

async fn adopt(hub: &Services, home: &PcId, out: &LandOutput) -> Result<()> {
    let theirs = out.conflicts.iter().flat_map(|c| c.theirs.iter().chain(c.base.iter()).cloned()).collect();
    hub.peers.pull(home, theirs).await?;
    match &out.change {
        Some(change) => hub.peers.refresh(home, &change.project).await,
        None => Ok(()),
    }
}
