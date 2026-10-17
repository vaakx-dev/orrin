use orrin_core::Result;
use orrin_core::model::{Artifact, Linked};

use crate::context::Context;
use crate::linked;
use crate::plan::Plan;

pub fn publish(ctx: &Context, plan: &Plan, artifacts: &[Artifact]) {
    if !plan.target.link_outputs {
        return;
    }
    match latest(ctx, plan, artifacts) {
        Ok(None) => {}
        Ok(Some(linked)) => {
            let (hub, project) = (ctx.hub.clone(), plan.project.clone());
            tokio::spawn(async move {
                if let Err(err) = hub.views.link(&project, linked).await {
                    tracing::warn!("could not link outputs into views of {project}: {err}");
                }
            });
        }
        Err(err) => tracing::warn!("could not keep the outputs of {} for linking: {err}", plan.name),
    }
}

fn latest(ctx: &Context, plan: &Plan, artifacts: &[Artifact]) -> Result<Option<Linked>> {
    let files = linked::files(&plan.target, artifacts)?;
    let linked = Linked { target: plan.name.clone(), files: ctx.hub.store.put_snapshot(&files)? };
    Ok(ctx.links.set(&plan.project, &linked)?.then_some(linked))
}
