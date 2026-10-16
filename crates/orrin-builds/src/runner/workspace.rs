use std::path::{Path, PathBuf};

use orrin_core::model::{Entry, Stream};
use orrin_core::{JobId, Result};

use super::blocking;
use super::folder;
use super::warm::{self, Lease};
use crate::context::Context;
use crate::plan::Plan;

pub enum Workspace {
    Fresh(PathBuf),
    Warm(Lease),
}

impl Workspace {
    pub async fn prepare(ctx: &Context, plan: &Plan, id: &JobId, files: Vec<Entry>) -> Result<Self> {
        let store = ctx.hub.store.clone();
        if plan.recipe.warm {
            let lease =
                ctx.warm.lease(&ctx.home.root().join("warm").join(plan.project.as_str()).join(safe(&plan.name)));
            let dir = lease.dir().to_path_buf();
            let synced = blocking(move || warm::sync(&*store, &dir, &files)).await?;
            let note = format!(
                "warm folder {}: {} written, {} unchanged, {} removed",
                lease.dir().display(),
                synced.written,
                synced.kept,
                synced.removed
            );
            ctx.table.log(id, Stream::System, note);
            return Ok(Self::Warm(lease));
        }
        let dir = folder::fresh(&ctx.home.runs(), id)?;
        let (target, big) = (dir.clone(), ctx.config.views.big_file);
        match blocking(move || folder::fill(&*store, &target, &files, big)).await {
            Ok(()) => Ok(Self::Fresh(dir)),
            Err(err) => {
                Self::Fresh(dir).close().await;
                Err(err)
            }
        }
    }

    pub fn dir(&self) -> &Path {
        match self {
            Self::Fresh(dir) => dir,
            Self::Warm(lease) => lease.dir(),
        }
    }

    pub async fn close(self) {
        if let Self::Fresh(dir) = self {
            let _ = tokio::task::spawn_blocking(move || folder::remove(&dir)).await;
        }
    }
}

fn safe(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
