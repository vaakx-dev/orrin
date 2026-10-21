use orrin_core::{Error, ProjectId, Result};

use crate::context::Context;
use crate::runner::warm_dir;

pub async fn remove_project(ctx: &Context, project: &ProjectId) -> Result<()> {
    if let Some(job) = ctx.table.jobs().into_iter().find(|job| job.project == *project && !job.state.is_done()) {
        return Err(Error::conflict(format!("job {} of {project} is still running; cancel it first", job.id)));
    }
    ctx.deliveries.remove(project);
    ctx.links.remove(project)?;
    let warm = warm_dir(ctx, project);
    ctx.warm.forget_under(&warm);
    let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&warm))
        .await
        .map_err(|err| Error::internal(err.to_string()))?;
    match removed {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
        _ => {}
    }
    ctx.table.remove_project(project);
    Ok(())
}
