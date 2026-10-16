use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use orrin_core::model::{Artifact, JobState, Stream};
use orrin_core::{Error, Hash, JobId, Result};

use crate::context::Context;
use crate::key::{self, Action};
use crate::plan::{Plan, tool_names};
use crate::tools;

type Boxed<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub fn key<'a>(ctx: &'a Context, plan: &'a Plan, name: &'a str, args: &'a [String]) -> Boxed<'a, Hash> {
    Box::pin(async move {
        let recipe = plan.recipes.get(name).ok_or_else(|| Error::not_found(format!("no recipe {name}")))?;
        let tools = tools::resolve(&ctx.hub, &tool_names(recipe)).await;
        let mut after = Vec::new();
        for dep in &recipe.after {
            after.push(key(ctx, plan, dep, &[]).await?);
        }
        key::action(Action { name, recipe, args, snapshot: &plan.snapshot, tools: &tools, after: &after })
    })
}

pub async fn outputs(ctx: &Arc<Context>, plan: &Plan, id: &JobId) -> Result<Vec<Artifact>> {
    let mut artifacts = Vec::new();
    for name in &plan.recipe.after {
        let job = submit(ctx.clone(), plan.sibling(name)?).await?;
        let how = if job.cached { "reused".to_string() } else { format!("job {}", job.id) };
        ctx.table.log(id, Stream::System, format!("after {name}: {how}"));
        let job = ctx.table.done(&job.id).await?;
        if job.state != JobState::Succeeded {
            return Err(Error::invalid(format!("after {name}: job {} did not succeed", job.id)));
        }
        artifacts.extend(ctx.table.artifacts(&job.id)?);
    }
    Ok(artifacts)
}

fn submit(ctx: Arc<Context>, plan: Plan) -> Boxed<'static, orrin_core::model::Job> {
    Box::pin(async move { super::submit(&ctx, plan).await })
}
