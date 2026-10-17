use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use orrin_core::model::{Artifact, Delivery, DeliveryState, JobKind, JobState, Snapshot};
use orrin_core::{Error, Result, now_ms};

use crate::context::Context;
use crate::plan::{Plan, Request};
use crate::runner::{self, SCREENSHOTS};

const RETRY: Duration = Duration::from_secs(2);

pub async fn run(ctx: Arc<Context>, mut delivery: Delivery, snapshot: Snapshot) {
    if let Err(err) = deliver(&ctx, &mut delivery, snapshot).await {
        delivery.state = DeliveryState::Failed;
        delivery.error = Some(err.message);
        delivery.finished_at = Some(now_ms());
        ctx.deliveries.set(&ctx, &delivery);
    }
}

async fn deliver(ctx: &Arc<Context>, delivery: &mut Delivery, snapshot: Snapshot) -> Result<()> {
    let mut pending = build(ctx, delivery, snapshot).await?;
    delivery.state = DeliveryState::Copying;
    ctx.deliveries.set(ctx, delivery);
    loop {
        let done = ctx.hub.views.deliver(&delivery.project, pending.clone()).await?;
        delivery.copied += done.copied;
        delivery.unchanged += done.unchanged;
        if done.busy.is_empty() {
            delivery.state = DeliveryState::Done;
            delivery.waiting.clear();
            delivery.finished_at = Some(now_ms());
            ctx.deliveries.set(ctx, delivery);
            return Ok(());
        }
        if delivery.waiting != done.busy {
            delivery.state = DeliveryState::Waiting;
            delivery.waiting = done.busy.clone();
            ctx.deliveries.set(ctx, delivery);
        }
        pending.retain(|file| done.busy.contains(&file.path));
        tokio::time::sleep(RETRY).await;
    }
}

async fn build(ctx: &Arc<Context>, delivery: &mut Delivery, snapshot: Snapshot) -> Result<Vec<Artifact>> {
    let mut jobs = Vec::new();
    for target in &delivery.targets {
        let request = Request {
            kind: JobKind::Build,
            project: delivery.project.clone(),
            snapshot_id: delivery.main.clone(),
            snapshot: snapshot.clone(),
            target: target.clone(),
            args: Vec::new(),
        };
        jobs.push(runner::submit(ctx, Plan::load(&*ctx.hub.store, request)?).await?);
    }
    delivery.jobs = jobs.iter().map(|job| job.id.clone()).collect();
    ctx.deliveries.set(ctx, delivery);
    let mut files = BTreeMap::new();
    for (target, job) in delivery.targets.iter().zip(&jobs) {
        let job = ctx.table.done(&job.id).await?;
        if job.state != JobState::Succeeded {
            return Err(Error::invalid(format!("{target} did not build (job {})", job.id)));
        }
        let made = ctx.table.artifacts(&job.id)?.into_iter().filter(|a| !a.path.starts_with(SCREENSHOTS));
        files.extend(made.map(|a| (a.path.clone(), a)));
    }
    Ok(files.into_values().collect())
}
