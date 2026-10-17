use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orrin_core::api::{JobArtifactsOutput, JobInput, JobList, JobListOutput, JobLogs, JobLogsOutput, JobRef, SyncJob};
use orrin_core::model::{Job, JobKind, Linked};
use orrin_core::service::{Builds, Service, Setup};
use orrin_core::{Error, Hash, ProjectId, Result};

use crate::context::Context;
use crate::plan::{Plan, Request};
use crate::remote::{self, Place};
use crate::roots;
use crate::{recover, runner, source};

const CANCEL_WAIT: Duration = Duration::from_secs(5);
const LOG_LIMIT: u32 = 1000;
const MAX_WAIT: Duration = Duration::from_secs(30);

pub struct BuildService {
    ctx: Arc<Context>,
}

impl BuildService {
    pub fn new(setup: &Setup) -> Result<Self> {
        Ok(Self { ctx: Arc::new(Context::new(setup)?) })
    }
}

impl Service for BuildService {
    fn start(&self) -> Result<()> {
        let runs = self.ctx.home.runs();
        if runs.exists() {
            std::fs::remove_dir_all(&runs)?;
        }
        std::fs::create_dir_all(runs)?;
        recover::restore(&self.ctx)
    }
}

#[async_trait]
impl Builds for BuildService {
    async fn submit(&self, kind: JobKind, input: JobInput) -> Result<Job> {
        let (snapshot_id, snapshot) = source::resolve(&self.ctx.hub, &input).await?;
        let request =
            Request { kind, project: input.project, snapshot_id, snapshot, target: input.target, args: input.args };
        let plan = Plan::load(&*self.ctx.hub.store, request)?;
        match remote::place(&self.ctx, &plan, input.pc.as_ref()).await? {
            Place::Local => runner::submit(&self.ctx, plan).await,
            Place::Peer(pc) => Ok(remote::submit(&self.ctx, plan, pc)),
        }
    }

    async fn accept(&self, input: SyncJob) -> Result<Job> {
        if let Some(job) = input.job {
            return match input.cancel {
                true => self.cancel(JobRef { job }).await,
                false => self.ctx.table.job(&job),
            };
        }
        if !self.ctx.builds_on().await {
            return Err(Error::unavailable("builds are off on this PC"));
        }
        let store = &*self.ctx.hub.store;
        let snapshot = store.snapshot(&input.snapshot)?;
        let hashes: Vec<_> = snapshot.entries.iter().map(|e| e.hash.clone()).collect();
        let missing = store.missing(&hashes).len();
        if missing > 0 {
            return Err(Error::not_found(format!("{missing} input files are missing")));
        }
        let request = Request {
            kind: input.kind,
            project: input.project,
            snapshot_id: input.snapshot,
            snapshot,
            target: input.target,
            args: input.args,
        };
        runner::submit(&self.ctx, Plan::load(store, request)?).await
    }

    async fn status(&self, input: JobRef) -> Result<Job> {
        self.ctx.table.job(&input.job)
    }

    async fn logs(&self, input: JobLogs) -> Result<JobLogsOutput> {
        let (limit, after) = (input.limit.unwrap_or(LOG_LIMIT) as usize, input.after.unwrap_or(0));
        match input.wait_ms.map(|ms| Duration::from_millis(ms).min(MAX_WAIT)) {
            Some(wait) => self.ctx.table.wait_logs(&input.job, after, limit, wait).await,
            None => self.ctx.table.logs(&input.job, after, limit),
        }
    }

    async fn artifacts(&self, input: JobRef) -> Result<JobArtifactsOutput> {
        Ok(JobArtifactsOutput { artifacts: self.ctx.table.artifacts(&input.job)? })
    }

    async fn cancel(&self, input: JobRef) -> Result<Job> {
        let job = self.ctx.table.job(&input.job)?;
        if job.state.is_done() {
            return Ok(job);
        }
        self.ctx.table.cancel(&input.job)?;
        self.ctx.table.wait_done(&input.job, CANCEL_WAIT).await
    }

    async fn list(&self, input: JobList) -> Result<JobListOutput> {
        let jobs =
            self.ctx.table.jobs().into_iter().filter(|job| input.project.as_ref().is_none_or(|p| *p == job.project));
        Ok(JobListOutput { jobs: jobs.collect() })
    }

    async fn roots(&self) -> Result<Vec<Hash>> {
        Ok(roots::collect(&self.ctx))
    }

    async fn linked(&self, project: &ProjectId) -> Result<Vec<Linked>> {
        Ok(self.ctx.links.all(project))
    }
}
