use std::sync::Arc;
use std::time::Duration;

use orrin_core::api::{PcsList, SyncFacts, SyncJob};
use orrin_core::model::{Job, JobResult, Stream};
use orrin_core::service::call_peer;
use orrin_core::{Error, Hash, JobId, PcId, Result, now_ms};
use tokio::sync::watch;

use crate::context::Context;
use crate::plan::Plan;
use crate::tools;

const POLL: Duration = Duration::from_secs(1);
const MAX_FAILURES: u32 = 5;

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

pub fn submit(ctx: &Arc<Context>, plan: Plan, pc: PcId) -> Job {
    let job = plan.job(pc.clone());
    let cancel = ctx.table.insert(job.clone());
    let follow = Follow { ctx: ctx.clone(), plan, pc, id: job.id.clone() };
    tokio::spawn(follow.go(cancel));
    job
}

struct Follow {
    ctx: Arc<Context>,
    plan: Plan,
    pc: PcId,
    id: JobId,
}

impl Follow {
    async fn go(self, cancel: watch::Receiver<bool>) {
        if let Err(err) = self.track(cancel).await {
            self.ctx.table.log(&self.id, Stream::System, err.to_string());
            self.ctx.table.update(&self.id, |job| {
                job.state = orrin_core::model::JobState::Failed;
                job.error = Some(err.message.clone());
                job.finished_at = Some(now_ms());
            });
        }
    }

    async fn track(&self, mut cancel: watch::Receiver<bool>) -> Result<()> {
        self.ctx.table.log(&self.id, Stream::System, format!("running on {}", self.pc));
        let mut hashes = vec![self.plan.snapshot_id.clone()];
        hashes.extend(self.plan.snapshot.entries.iter().map(|e| e.hash.clone()));
        self.ctx.hub.peers.push(&self.pc, hashes).await?;
        let request = SyncJob {
            kind: self.plan.kind,
            project: self.plan.project.clone(),
            snapshot: self.plan.snapshot_id.clone(),
            recipe: self.plan.name.clone(),
            args: self.plan.args.clone(),
            job: None,
            cancel: false,
        };
        let mut remote: Job = call_peer(&*self.ctx.hub.peers, &self.pc, request.clone()).await?;
        let (mut failures, mut cancel_sent) = (0, false);
        while !remote.state.is_done() {
            self.mirror(&remote, None);
            let cancelling = match cancel_sent {
                true => {
                    tokio::time::sleep(POLL).await;
                    false
                }
                false => tokio::select! {
                    _ = tokio::time::sleep(POLL) => false,
                    _ = cancel.wait_for(|c| *c) => true,
                },
            };
            cancel_sent |= cancelling;
            let poll = SyncJob { job: Some(remote.id.clone()), cancel: cancelling, ..request.clone() };
            match call_peer(&*self.ctx.hub.peers, &self.pc, poll).await {
                Ok(job) => (remote, failures) = (job, 0),
                Err(err) if failures + 1 >= MAX_FAILURES => return Err(err),
                Err(_) => failures += 1,
            }
        }
        if let Some(result) = &remote.result {
            self.fetch(result).await?;
        }
        self.mirror(&remote, remote.result.clone());
        Ok(())
    }

    async fn fetch(&self, result: &Hash) -> Result<()> {
        self.ctx.hub.peers.pull(&self.pc, vec![result.clone()]).await?;
        let result: JobResult = serde_json::from_slice(&self.ctx.hub.store.get(result)?)?;
        let hashes = result.artifacts.iter().map(|a| a.hash.clone()).collect();
        self.ctx.hub.peers.pull(&self.pc, hashes).await?;
        self.ctx.table.replay(&self.id, result.logs);
        self.ctx.table.set_artifacts(&self.id, result.artifacts);
        Ok(())
    }

    fn mirror(&self, remote: &Job, result: Option<Hash>) {
        self.ctx.table.update(&self.id, |job| {
            job.state = remote.state;
            job.cached = remote.cached;
            job.exit_code = remote.exit_code;
            job.error = remote.error.clone();
            job.started_at = remote.started_at;
            job.finished_at = remote.finished_at;
            job.result = result;
        });
    }
}
