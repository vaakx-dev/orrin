mod folder;
mod outputs;

use std::path::PathBuf;
use std::sync::Arc;

use orrin_core::model::{Artifact, Job, JobResult, JobState, Stream};
use orrin_core::{Error, Hash, JobId, Result, now_ms};
use tokio::sync::watch;

use crate::context::Context;
use crate::key;
use crate::plan::{Plan, tool_names};
use crate::process::{self, Exit, Limits, Spec};
use crate::queue::Need;
use crate::table::Claim;
use crate::tools::{self, Tools};

const RESULT_LINES: usize = 2000;

pub async fn submit(ctx: &Arc<Context>, plan: Plan) -> Result<Job> {
    let tools = tools::resolve(&ctx.hub, &plan.tools()).await;
    let key = key::action(&plan.name, &plan.recipe, &plan.args, &plan.snapshot, &tools)?;
    if plan.cacheable()
        && let Some((hash, result)) = ctx.cache.lookup(&*ctx.hub.store, &key)
    {
        return Ok(cached(ctx, &plan, hash, result));
    }
    let job = plan.job(ctx.me.pc.clone());
    let (job, cancel) = match plan.cacheable() {
        false => {
            let cancel = ctx.table.insert(job.clone());
            (job, cancel)
        }
        true => match ctx.table.claim(job, &key) {
            Claim::Shared(job) => return Ok(job),
            Claim::Fresh(job, cancel) => (job, cancel),
        },
    };
    let run = Run { ctx: ctx.clone(), plan: Arc::new(plan), tools, key, id: job.id.clone() };
    tokio::spawn(run.go(cancel));
    Ok(job)
}

fn cached(ctx: &Context, plan: &Plan, hash: Hash, result: JobResult) -> Job {
    let now = now_ms();
    let mut job = plan.job(ctx.me.pc.clone());
    job.state = JobState::Succeeded;
    job.cached = true;
    job.exit_code = result.exit_code;
    job.started_at = Some(now);
    job.finished_at = Some(now);
    job.result = Some(hash);
    ctx.table.insert(job.clone());
    ctx.table.set_artifacts(&job.id, result.artifacts);
    ctx.table.replay(&job.id, result.logs);
    job
}

struct Run {
    ctx: Arc<Context>,
    plan: Arc<Plan>,
    tools: Tools,
    key: Hash,
    id: JobId,
}

impl Run {
    async fn go(self, mut cancel: watch::Receiver<bool>) {
        let recipe = &self.plan.recipe;
        let need = Need {
            cpu: recipe.cpu.unwrap_or(1),
            ram_mb: recipe.ram_mb.unwrap_or(0),
            gpu: self.plan.needs_gpu() as u32,
        };
        let slot = tokio::select! {
            slot = self.ctx.queue.acquire(need) => Some(slot),
            _ = cancel.wait_for(|c| *c) => None,
        };
        match slot {
            None => self.cancelled(),
            Some(slot) => {
                self.ctx.table.update(&self.id, |job| {
                    job.state = JobState::Running;
                    job.started_at = Some(now_ms());
                });
                let outcome = self.execute(cancel).await;
                drop(slot);
                self.finish(outcome).await;
            }
        }
        self.ctx.table.release(&self.key, &self.id);
    }

    async fn execute(&self, cancel: watch::Receiver<bool>) -> Result<(Exit, Vec<Artifact>)> {
        let dir = folder::fresh(&self.ctx.home.runs(), &self.id)?;
        let outcome = self.execute_in(&dir, cancel).await;
        let cleanup = dir.clone();
        let _ = tokio::task::spawn_blocking(move || folder::remove(&cleanup)).await;
        outcome
    }

    async fn execute_in(&self, dir: &std::path::Path, cancel: watch::Receiver<bool>) -> Result<(Exit, Vec<Artifact>)> {
        let extras = self.cached_outputs().await;
        let (store, plan, target) = (self.ctx.hub.store.clone(), self.plan.clone(), dir.to_path_buf());
        let big = self.ctx.config.views.big_file;
        blocking(move || folder::fill(&*store, &target, &plan.snapshot, big, &extras)).await?;
        self.ctx.table.log(&self.id, Stream::System, format!("$ {}", self.plan.recipe.command));
        let exit =
            process::run(self.spec(dir), cancel, |stream, text| self.ctx.table.log(&self.id, stream, text)).await?;
        if exit != Exit::Code(Some(0)) {
            return Ok((exit, Vec::new()));
        }
        let (store, plan, target) = (self.ctx.hub.store.clone(), self.plan.clone(), dir.to_path_buf());
        let artifacts =
            blocking(move || outputs::collect(&*store, &target, &plan.recipe.outputs, &plan.snapshot)).await?;
        Ok((exit, artifacts))
    }

    fn spec(&self, dir: &std::path::Path) -> Spec {
        let recipe = &self.plan.recipe;
        let mut env = vec![
            ("ORRIN_JOB".to_string(), self.id.as_str().into()),
            ("ORRIN_RECIPE".to_string(), self.plan.name.clone().into()),
            ("ORRIN_SCREENSHOTS".to_string(), dir.join(folder::SCREENSHOTS).into_os_string()),
        ];
        if let Some(path) = tools::search_path(&self.tools) {
            env.push(("PATH".to_string(), path));
        }
        let cores = self.ctx.queue.cores();
        let limits =
            Limits { cpu_percent: recipe.cpu.map(|c| c.saturating_mul(100) / cores), memory_mb: recipe.ram_mb };
        Spec { command: recipe.command.clone(), args: self.plan.args.clone(), dir: PathBuf::from(dir), env, limits }
    }

    async fn cached_outputs(&self) -> Vec<Artifact> {
        let mut artifacts = Vec::new();
        for (name, recipe) in self.plan.recipes.iter().filter(|(name, _)| **name != self.plan.name) {
            let tools = tools::resolve(&self.ctx.hub, &tool_names(recipe)).await;
            let Ok(key) = key::action(name, recipe, &[], &self.plan.snapshot, &tools) else { continue };
            if let Some((_, result)) = self.ctx.cache.lookup(&*self.ctx.hub.store, &key) {
                artifacts.extend(result.artifacts);
            }
        }
        artifacts
    }

    async fn finish(&self, outcome: Result<(Exit, Vec<Artifact>)>) {
        let stored = match outcome {
            Ok((Exit::Cancelled, _)) => return self.cancelled(),
            Ok((Exit::Code(code), artifacts)) => self.store_result(code, artifacts).await.map(|hash| (code, hash)),
            Err(err) => Err(err),
        };
        match stored {
            Ok((code, hash)) => {
                let ok = code == Some(0);
                self.ctx.table.update(&self.id, |job| {
                    job.state = if ok { JobState::Succeeded } else { JobState::Failed };
                    job.exit_code = code;
                    job.error =
                        (!ok).then(|| format!("exited with code {}", code.map_or("none".into(), |c| c.to_string())));
                    job.result = Some(hash);
                    job.finished_at = Some(now_ms());
                });
            }
            Err(err) => {
                self.ctx.table.log(&self.id, Stream::System, err.to_string());
                self.ctx.table.update(&self.id, |job| {
                    job.state = JobState::Failed;
                    job.error = Some(err.message.clone());
                    job.finished_at = Some(now_ms());
                });
            }
        }
    }

    async fn store_result(&self, code: Option<i32>, artifacts: Vec<Artifact>) -> Result<Hash> {
        let ok = code == Some(0);
        let result = JobResult {
            exit_code: code,
            artifacts: artifacts.clone(),
            logs: self.ctx.table.tail(&self.id, RESULT_LINES),
        };
        let hash = self.ctx.hub.store.put(&serde_json::to_vec(&result)?)?;
        self.ctx.table.set_artifacts(&self.id, artifacts);
        if ok && self.plan.cacheable() {
            self.ctx.cache.put(&self.key, &hash)?;
        }
        if ok {
            tools::learn(&self.ctx.hub, &self.plan.recipe.command, &self.tools).await;
        }
        Ok(hash)
    }

    fn cancelled(&self) {
        self.ctx.table.update(&self.id, |job| {
            job.state = JobState::Cancelled;
            job.finished_at = Some(now_ms());
        });
    }
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|err| Error::internal(err.to_string()))?
}
