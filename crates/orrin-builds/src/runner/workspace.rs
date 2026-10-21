use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use orrin_core::model::{Entry, Snapshot, Stream};
use orrin_core::{Hash, JobId, ProjectId, Result};

use super::folder;
use super::warm::{Carried, Lease};
use super::{after, blocking};
use crate::context::Context;
use crate::plan::Plan;

pub enum Workspace {
    Fresh(PathBuf),
    Warm(Lease),
}

impl Workspace {
    pub async fn prepare(
        ctx: &Context,
        plan: &Plan,
        id: &JobId,
        files: Arc<Snapshot>,
        linked: Arc<HashSet<String>>,
    ) -> Result<Self> {
        let store = ctx.hub.store.clone();
        if let Some(size) = plan.target.warm.pool(ctx.config.builds.warm_pool) {
            let root = warm_root(ctx, plan, &plan.name);
            if ctx.warm.is_full(&root, size as usize) {
                ctx.table.log(id, Stream::System, format!("waiting for one of {size} warm folders"));
            }
            let mut lease = ctx.warm.lease(&root, &files.entries, size as usize).await;
            let (rewrite, count) = (lease.rewrite, lease.folders);
            let (lease, synced) = blocking(move || {
                let synced = lease.sync(&*store, &files.entries, &linked)?;
                Ok((lease, synced))
            })
            .await?;
            let note = format!(
                "warm folder {} (closest of {count}, {rewrite} to rewrite): {} written, {} unchanged, {} removed",
                lease.number(),
                synced.written,
                synced.kept,
                synced.removed
            );
            ctx.table.log(id, Stream::System, note);
            return Ok(Self::Warm(lease));
        }
        let dir = folder::fresh(&ctx.home.runs(), id)?;
        let (target, big) = (dir.clone(), ctx.config.views.big_file);
        match blocking(move || folder::fill(&*store, &target, &files.entries, &linked, big)).await {
            Ok(()) => Ok(Self::Fresh(dir)),
            Err(err) => {
                Self::Fresh(dir).close();
                Err(err)
            }
        }
    }

    pub async fn share(&self, ctx: &Context, plan: &Plan, id: &JobId, files: &[Entry]) -> Result<()> {
        if plan.target.share.is_empty() {
            return Ok(());
        }
        for name in &plan.target.after {
            let key = after::key(ctx, plan, name, &[]).await?;
            let Some(from) = ctx.warm.lease_built(&warm_root(ctx, plan, name), &key) else {
                let note = format!("share: no idle warm folder of {name} holds this build; nothing carried");
                ctx.table.log(id, Stream::System, note);
                continue;
            };
            let (to, patterns) = (self.dir().to_path_buf(), plan.target.share.clone());
            let paths: Vec<String> = files.iter().map(|e| e.path.clone()).collect();
            let carried = blocking(move || {
                let skip: HashSet<&str> = paths.iter().map(String::as_str).collect();
                super::warm::carry(from.dir(), &to, &patterns, &skip)
            })
            .await?;
            let Carried { copied, kept } = carried;
            ctx.table.log(id, Stream::System, format!("share from {name}: {copied} copied, {kept} already in place"));
        }
        Ok(())
    }

    pub fn built(&mut self, key: &Hash) {
        if let Self::Warm(lease) = self {
            lease.built(key);
        }
    }

    pub fn dir(&self) -> &Path {
        match self {
            Self::Fresh(dir) => dir,
            Self::Warm(lease) => lease.dir(),
        }
    }

    pub fn close(self) {
        if let Self::Fresh(dir) = self {
            folder::discard(dir);
        }
    }
}

fn warm_root(ctx: &Context, plan: &Plan, target: &str) -> PathBuf {
    warm_dir(ctx, &plan.project).join(safe(target))
}

pub fn warm_dir(ctx: &Context, project: &ProjectId) -> PathBuf {
    ctx.home.root().join("warm").join(project.as_str())
}

fn safe(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
