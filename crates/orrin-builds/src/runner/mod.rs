mod after;
mod compiler;
mod folder;
mod outputs;
mod placeholders;
mod publish;
pub mod steps;
pub mod warm;
mod workspace;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use orrin_core::model::{Artifact, Entry, Job, JobResult, JobState, Snapshot, Stream};
use orrin_core::{Error, Hash, JobId, Result, now_ms};
use tokio::sync::watch;

use crate::context::Context;
use crate::plan::{Plan, Request};
use crate::process::{self, Exit, Limits, Spec};
use crate::queue::{Need, Slot};
use crate::table::Claim;
use crate::tools::{self, Tools};
use placeholders::Values;
use workspace::Workspace;

const RESULT_LINES: usize = 2000;

pub async fn submit(ctx: &Arc<Context>, plan: Plan) -> Result<Job> {
    let tools = tools::resolve(&ctx.hub, &plan.tools()).await;
    let key = after::key(ctx, &plan, &plan.name, &plan.args).await?;
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
    ctx.table.journal().args(&job.id, &plan.args);
    let run = Run { ctx: ctx.clone(), plan: Arc::new(plan), tools, key, id: job.id.clone() };
    tokio::spawn(run.go(cancel));
    Ok(job)
}

pub async fn resume(ctx: &Arc<Context>, job: &Job, args: Vec<String>, cancel: watch::Receiver<bool>) -> Result<()> {
    let store = &*ctx.hub.store;
    let request = Request {
        kind: job.kind,
        project: job.project.clone(),
        snapshot_id: job.snapshot.clone(),
        snapshot: store.snapshot(&job.snapshot)?,
        target: job.target.clone(),
        args,
    };
    let plan = Plan::load(store, request)?;
    let tools = tools::resolve(&ctx.hub, &plan.tools()).await;
    let key = after::key(ctx, &plan, &plan.name, &plan.args).await?;
    Run { ctx: ctx.clone(), plan: Arc::new(plan), tools, key, id: job.id.clone() }.go(cancel).await;
    Ok(())
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
    publish::publish(ctx, plan, &result.artifacts);
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
        match self.ready(&mut cancel).await {
            None => self.cancelled(),
            Some(Err(err)) => self.finish(Err(err)).await,
            Some(Ok((extras, slot))) => {
                self.ctx.table.update(&self.id, |job| {
                    job.state = JobState::Running;
                    job.started_at = Some(now_ms());
                });
                let outcome = self.execute(extras, cancel).await;
                drop(slot);
                self.finish(outcome).await;
            }
        }
        self.ctx.table.release(&self.key, &self.id);
    }

    async fn ready(&self, cancel: &mut watch::Receiver<bool>) -> Option<Result<(after::Outputs, Slot)>> {
        let target = &self.plan.target;
        let need = Need {
            cpu: self.plan.cpu(self.ctx.queue.cores()),
            ram_mb: target.ram_mb.unwrap_or(0),
            gpu: self.plan.needs_gpu() as u32,
        };
        let work = async {
            let extras = after::outputs(&self.ctx, &self.plan, &self.id).await?;
            Ok((extras, self.ctx.queue.acquire(need).await))
        };
        tokio::select! {
            ready = work => Some(ready),
            _ = cancel.wait_for(|c| *c) => None,
        }
    }

    async fn execute(
        &self,
        extras: after::Outputs,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<(Exit, Vec<Artifact>)> {
        let files = Arc::new(inputs(&self.plan.snapshot, extras.files));
        let prepare = Workspace::prepare(&self.ctx, &self.plan, &self.id, files.clone(), Arc::new(extras.linked));
        let mut workspace = tokio::select! {
            workspace = prepare => workspace?,
            _ = cancel.wait_for(|c| *c) => return Ok((Exit::Cancelled, Vec::new())),
        };
        let outcome = match workspace.share(&self.ctx, &self.plan, &self.id, &files.entries).await {
            Ok(()) => self.execute_in(workspace.dir(), files, cancel).await,
            Err(err) => Err(err),
        };
        if matches!(outcome, Ok((Exit::Code(Some(0)), _))) {
            workspace.built(&self.key);
        }
        workspace.close();
        outcome
    }

    async fn execute_in(
        &self,
        dir: &Path,
        files: Arc<Snapshot>,
        cancel: watch::Receiver<bool>,
    ) -> Result<(Exit, Vec<Artifact>)> {
        let env = self.env(dir).await?;
        let mut target = self.plan.target.clone();
        if !target.steps.is_empty() {
            let built = steps::build(steps::Build {
                ctx: &self.ctx,
                plan: &self.plan,
                tools: &self.tools,
                id: &self.id,
                dir,
                files: &files,
                env: &env,
                parallel: self.plan.cpu(self.ctx.queue.cores()),
                cancel: cancel.clone(),
            })
            .await?;
            if built.exit != Exit::Code(Some(0)) {
                return Ok((built.exit, Vec::new()));
            }
            if target.outputs.is_empty() {
                target.outputs = built.outputs.iter().map(|path| globset::escape(path)).collect();
            }
        }
        if !target.command.is_empty() {
            let spec = self.spec(dir, env);
            self.ctx.table.log(&self.id, Stream::System, format!("$ {}", spec.command));
            let exit = process::run(spec, cancel, |stream, text| self.ctx.table.log(&self.id, stream, text)).await?;
            if exit != Exit::Code(Some(0)) {
                return Ok((exit, Vec::new()));
            }
        }
        let (store, folder) = (self.ctx.hub.store.clone(), dir.to_path_buf());
        let artifacts = blocking(move || outputs::collect(&*store, &folder, &target, &files)).await?;
        Ok((Exit::Code(Some(0)), artifacts))
    }

    async fn env(&self, dir: &Path) -> Result<compiler::Env> {
        let screenshots = dir.join(folder::SCREENSHOTS);
        let mut env = vec![
            ("ORRIN_JOB".to_string(), self.id.as_str().into()),
            ("ORRIN_TARGET".to_string(), self.plan.name.clone().into()),
            ("ORRIN_SCREENSHOTS".to_string(), screenshots.clone().into_os_string()),
        ];
        if let Some(path) = tools::search_path(&self.tools) {
            env.push(("PATH".to_string(), path));
        }
        let wiring = compiler::wire(&self.ctx, &self.plan.target, dir).await?;
        if let Some(note) = wiring.note {
            self.ctx.table.log(&self.id, Stream::System, note);
        }
        env.extend(wiring.env);
        Ok(env)
    }

    fn spec(&self, dir: &Path, env: compiler::Env) -> Spec {
        let target = &self.plan.target;
        let screenshots = dir.join(folder::SCREENSHOTS);
        let values = Values {
            dir,
            screenshots: &screenshots,
            job: self.id.as_str(),
            target: &self.plan.name,
            args: &self.plan.args,
        };
        let expanded = placeholders::expand(&tools::command_line(&target.command, &self.tools), &values);
        let cores = self.ctx.queue.cores();
        let limits =
            Limits { cpu_percent: target.cpu.map(|c| c.saturating_mul(100) / cores), memory_mb: target.ram_mb };
        Spec {
            command: expanded.command,
            args: if expanded.took_args { Vec::new() } else { self.plan.args.clone() },
            dir: dir.to_path_buf(),
            env,
            limits,
        }
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
                if ok {
                    let (ctx, plan, tools) = (self.ctx.clone(), self.plan.clone(), self.tools.clone());
                    tokio::spawn(async move { tools::learn(&ctx.hub, &plan.target.command, &tools).await });
                }
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
            publish::publish(&self.ctx, &self.plan, &result.artifacts);
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

fn inputs(snapshot: &Snapshot, extras: Vec<Artifact>) -> Snapshot {
    let mut files: BTreeMap<String, Entry> = snapshot.entries.iter().map(|e| (e.path.clone(), e.clone())).collect();
    for artifact in extras {
        let entry =
            Entry { path: artifact.path.clone(), hash: artifact.hash, size: artifact.size, mode: Default::default() };
        files.insert(artifact.path, entry);
    }
    Snapshot::new(files.into_values().collect())
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|err| Error::internal(err.to_string()))?
}
