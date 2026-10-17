use std::sync::Arc;
use std::time::{Duration, Instant};

use orrin_core::api::{JobLogs, JobLogsOutput, SyncJob, SyncLogs};
use orrin_core::model::{Job, JobResult, JobState, Stream};
use orrin_core::service::call_peer;
use orrin_core::{Hash, JobId, PcId, Result, now_ms};
use tokio::sync::watch;

use crate::context::Context;
use crate::plan::Plan;

const RETRY: Duration = Duration::from_secs(1);
const MIRROR_EVERY: Duration = Duration::from_secs(1);
const WAIT_MS: u64 = 25_000;
const MAX_FAILURES: u32 = 5;
const PAGE: u32 = 1000;
const OWN_LINES: u64 = 1;

pub fn submit(ctx: &Arc<Context>, plan: Plan, pc: PcId) -> Job {
    let job = plan.job(pc.clone());
    let cancel = ctx.table.insert(job.clone());
    ctx.table.journal().args(&job.id, &plan.args);
    let request = SyncJob {
        kind: plan.kind,
        project: plan.project.clone(),
        snapshot: plan.snapshot_id.clone(),
        recipe: plan.name.clone(),
        args: plan.args.clone(),
        job: None,
        cancel: false,
    };
    let follow = Follow { ctx: ctx.clone(), request, pc, id: job.id.clone() };
    tokio::spawn(async move {
        let outcome = match follow.start(&plan).await {
            Ok(remote) => follow.track(remote, 0, cancel).await,
            Err(err) => Err(err),
        };
        follow.end(outcome);
    });
    job
}

pub fn resume(ctx: &Arc<Context>, job: &Job, args: Vec<String>, remote: JobId, cancel: watch::Receiver<bool>) {
    let request = SyncJob {
        kind: job.kind,
        project: job.project.clone(),
        snapshot: job.snapshot.clone(),
        recipe: job.recipe.clone(),
        args,
        job: None,
        cancel: false,
    };
    let seen = ctx.table.tail(&job.id, 1).first().map_or(0, |line| line.n.saturating_sub(OWN_LINES));
    let follow = Follow { ctx: ctx.clone(), request, pc: job.pc.clone(), id: job.id.clone() };
    tokio::spawn(async move {
        let outcome = follow.track(remote, seen, cancel).await;
        follow.end(outcome);
    });
}

struct Follow {
    ctx: Arc<Context>,
    request: SyncJob,
    pc: PcId,
    id: JobId,
}

struct Cursor {
    after: u64,
    live: bool,
    mirrored: Instant,
}

impl Follow {
    async fn start(&self, plan: &Plan) -> Result<JobId> {
        self.ctx.table.log(&self.id, Stream::System, format!("running on {}", self.pc));
        let mut hashes = vec![plan.snapshot_id.clone()];
        hashes.extend(plan.snapshot.entries.iter().map(|e| e.hash.clone()));
        self.ctx.hub.peers.push(&self.pc, hashes).await?;
        let remote: Job = call_peer(&*self.ctx.hub.peers, &self.pc, self.request.clone()).await?;
        self.ctx.table.journal().remote(&self.id, &remote.id);
        if !remote.state.is_done() {
            self.mirror(&remote, None);
        }
        Ok(remote.id)
    }

    async fn track(&self, remote: JobId, seen: u64, mut cancel: watch::Receiver<bool>) -> Result<()> {
        let mut cursor = Cursor { after: seen, live: false, mirrored: Instant::now() };
        let (mut failures, mut cancel_sent) = (0, false);
        loop {
            let page = tokio::select! {
                page = self.page(&remote, cursor.after) => page,
                _ = cancelled(&mut cancel), if !cancel_sent => {
                    cancel_sent = true;
                    let _ = self.status(&remote, true).await;
                    continue;
                }
            };
            let page = match page {
                Ok(page) => page,
                Err(err) => {
                    failures += 1;
                    if failures >= MAX_FAILURES {
                        return Err(err);
                    }
                    tokio::time::sleep(RETRY).await;
                    continue;
                }
            };
            failures = 0;
            cursor.live = true;
            let quiet = page.lines.is_empty();
            let full = page.lines.len() >= PAGE as usize;
            for line in page.lines {
                cursor.after = line.n;
                self.ctx.table.log(&self.id, line.stream, line.text);
            }
            if page.done && !full {
                let job = self.status(&remote, false).await?;
                return self.done(job, &cursor).await;
            }
            if quiet || cursor.mirrored.elapsed() >= MIRROR_EVERY {
                cursor.mirrored = Instant::now();
                if let Ok(job) = self.status(&remote, false).await
                    && !job.state.is_done()
                {
                    self.mirror(&job, None);
                }
            }
        }
    }

    async fn page(&self, remote: &JobId, after: u64) -> Result<JobLogsOutput> {
        let ask = JobLogs { job: remote.clone(), after: Some(after), limit: Some(PAGE), wait_ms: Some(WAIT_MS) };
        call_peer(&*self.ctx.hub.peers, &self.pc, SyncLogs(ask)).await
    }

    async fn status(&self, remote: &JobId, cancel: bool) -> Result<Job> {
        let ask = SyncJob { job: Some(remote.clone()), cancel, ..self.request.clone() };
        call_peer(&*self.ctx.hub.peers, &self.pc, ask).await
    }

    async fn done(&self, remote: Job, cursor: &Cursor) -> Result<()> {
        if let Some(result) = &remote.result {
            self.fetch(result, cursor.live).await?;
        }
        self.mirror(&remote, remote.result.clone());
        Ok(())
    }

    async fn fetch(&self, result: &Hash, streamed: bool) -> Result<()> {
        self.ctx.hub.peers.pull(&self.pc, vec![result.clone()]).await?;
        let result: JobResult = serde_json::from_slice(&self.ctx.hub.store.get(result)?)?;
        let hashes = result.artifacts.iter().map(|a| a.hash.clone()).collect();
        self.ctx.hub.peers.pull(&self.pc, hashes).await?;
        if !streamed {
            self.ctx.table.replay(&self.id, result.logs);
        }
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

    fn end(&self, outcome: Result<()>) {
        let Err(err) = outcome else { return };
        self.ctx.table.log(&self.id, Stream::System, err.to_string());
        self.ctx.table.update(&self.id, |job| {
            job.state = JobState::Failed;
            job.error = Some(err.message.clone());
            job.finished_at = Some(now_ms());
        });
    }
}

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    let _ = cancel.wait_for(|c| *c).await;
}
