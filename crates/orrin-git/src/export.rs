use std::sync::Arc;

use orrin_core::api::GitExportOutput;
use orrin_core::model::{Change, Project};
use orrin_core::service::{Hub, Store};
use orrin_core::{Error, Result};

use crate::mapping::Mapping;
use crate::target::Target;
use crate::{history, remote, repo};

pub async fn run(hub: &Hub, project: Project, target: Target) -> Result<GitExportOutput> {
    let changes = history::oldest_first(hub, &project.id).await?;
    let store = hub.store.clone();
    tokio::task::spawn_blocking(move || export(store, &target, changes))
        .await
        .map_err(|err| Error::internal(err.to_string()))?
}

fn export(store: Arc<dyn Store>, target: &Target, changes: Vec<Change>) -> Result<GitExportOutput> {
    let repo = repo::open(&target.repo)?;
    let mut mapping = Mapping::load(&target.mapping)?;
    let last = changes.iter().rposition(|c| mapping.commit_of(&c.id).is_some());
    let mut head = match last {
        Some(index) => Some(repo::parse(mapping.commit_of(&changes[index].id).unwrap_or_default())?),
        None => repo::target(&repo, &target.branch_ref())?,
    };
    let pending = &changes[last.map_or(0, |i| i + 1)..];
    for change in pending {
        let snapshot = store.snapshot(&change.snapshot)?;
        let tree = repo::write_snapshot(&repo, store.as_ref(), &mut mapping, &snapshot)?;
        let commit = repo::write_commit(&repo, tree, head, change)?;
        mapping.link(change.id.clone(), commit.to_string());
        head = Some(commit);
    }
    if let Some(head) = head {
        repo::set(&repo, &target.branch_ref(), head)?;
    }
    mapping.save()?;
    if let (Some(remote), Some(_)) = (&target.remote, head) {
        remote::push(&target.repo, remote, &target.branch_ref())?;
    }
    Ok(GitExportOutput { commits: pending.len() as u32, head: head.map(|h| h.to_string()) })
}
