mod follow;

use orrin_core::api::{PcsList, SyncFacts};
use orrin_core::service::call_peer;
use orrin_core::{Error, PcId, Result};

use crate::context::Context;
use crate::plan::Plan;
use crate::tools;

pub use follow::{resume, submit};

pub enum Place {
    Local,
    Peer(PcId),
}

pub async fn place(ctx: &Context, plan: &Plan, asked: Option<&PcId>) -> Result<Place> {
    if let Some(pc) = asked {
        return Ok(if *pc == ctx.me.pc { Place::Local } else { Place::Peer(pc.clone()) });
    }
    let names = plan.tools();
    let known = tools::all_known(&tools::resolve(&ctx.hub, &names).await);
    let gpu = !plan.needs_gpu() || ctx.config.builds.gpu > 0;
    let builds = ctx.builds_on().await;
    if builds && known && gpu {
        return Ok(Place::Local);
    }
    if let Some(pc) = pick(ctx, &names).await {
        return Ok(Place::Peer(pc));
    }
    match builds {
        true => Ok(Place::Local),
        false => Err(Error::unavailable(format!("builds are off on this PC and no paired PC can run {}", plan.name))),
    }
}

async fn pick(ctx: &Context, needs: &[String]) -> Option<PcId> {
    let pcs = ctx.hub.peers.pcs(PcsList {}).await.ok()?.pcs;
    for pc in pcs.into_iter().filter(|p| !p.me && p.online && p.settings.builds) {
        let Ok(facts) = call_peer(&*ctx.hub.peers, &pc.id, SyncFacts {}).await else { continue };
        if needs.iter().all(|n| facts.tools.iter().any(|t| t.name.eq_ignore_ascii_case(n))) {
            return Some(pc.id);
        }
    }
    None
}
