use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::try_join_all;
use orrin_core::model::{Artifact, Job, JobState, Stream};
use orrin_core::{Error, Hash, JobId, Result};

use crate::context::Context;
use crate::key::{self, Action};
use crate::{linked, manifest};
use crate::plan::{Plan, tool_names};
use crate::tools;

type Boxed<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

const PAGE: usize = 1000;
const WAIT: Duration = Duration::from_secs(30);

pub fn key<'a>(ctx: &'a Context, plan: &'a Plan, name: &'a str, args: &'a [String]) -> Boxed<'a, Hash> {
    Box::pin(async move {
        let target = manifest::target(&plan.manifest, name)?;
        let tools = tools::resolve(&ctx.hub, &tool_names(target)).await;
        let mut after = Vec::new();
        for dep in &target.after {
            after.push(key(ctx, plan, dep, &[]).await?);
        }
        after.extend(super::packages::keys(ctx, plan, target).await?);
        key::action(Action { name, target, args, snapshot: &plan.snapshot, tools: &tools, after: &after })
    })
}

#[derive(Default)]
pub struct Outputs {
    pub files: Vec<Artifact>,
    pub linked: HashSet<String>,
}

pub async fn outputs(ctx: &Arc<Context>, plan: &Plan, id: &JobId) -> Result<Outputs> {
    let mut started = Vec::new();
    for name in &plan.target.after {
        let job = submit(ctx.clone(), plan.sibling(name)?).await?;
        let how = if job.cached { "reused".to_string() } else { format!("job {}", job.id) };
        ctx.table.log(id, Stream::System, format!("after {name}: {how}"));
        started.push((name.as_str(), job));
    }
    let jobs = try_join_all(started.iter().map(|(name, job)| relay(ctx, id, name, job))).await?;
    let mut outputs = Outputs::default();
    for (name, job) in plan.target.after.iter().zip(jobs) {
        if job.state != JobState::Succeeded {
            return Err(Error::invalid(format!("after {name}: job {} did not succeed", job.id)));
        }
        let artifacts = ctx.table.artifacts(&job.id)?;
        let target = manifest::target(&plan.manifest, name)?;
        if target.link_outputs {
            let files = linked::files(target, &artifacts)?;
            outputs.linked.extend(files.entries.into_iter().map(|e| e.path));
        }
        outputs.files.extend(artifacts);
    }
    Ok(outputs)
}

async fn relay(ctx: &Context, parent: &JobId, name: &str, job: &Job) -> Result<Job> {
    if job.cached {
        return Ok(job.clone());
    }
    let mut after = 0;
    loop {
        let page = ctx.table.wait_logs(&job.id, after, PAGE, WAIT).await?;
        let full = page.lines.len() >= PAGE;
        for line in page.lines {
            after = line.n;
            ctx.table.log(parent, line.stream, format!("[{name}] {}", line.text));
        }
        if page.done && !full {
            return ctx.table.job(&job.id);
        }
    }
}

fn submit(ctx: Arc<Context>, plan: Plan) -> Boxed<'static, Job> {
    Box::pin(async move { super::submit(&ctx, plan).await })
}
