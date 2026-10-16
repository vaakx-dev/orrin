use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use orrin_core::api::{GitImportOutput, SyncLand};
use orrin_core::model::{FileState, Project, Snapshot};
use orrin_core::service::{Hub, Store};
use orrin_core::{ChangeId, Error, Result, SnapshotId};

use crate::mapping::Mapping;
use crate::repo::Imported;
use crate::target::Target;
use crate::{history, remote, repo};

pub async fn run(hub: &Hub, project: Project, target: Target) -> Result<GitImportOutput> {
    let store = hub.store.clone();
    let fetch = target.clone();
    let commits = tokio::task::spawn_blocking(move || fetch_new(store, &fetch))
        .await
        .map_err(|err| Error::internal(err.to_string()))??;
    let mut known: HashMap<SnapshotId, ChangeId> =
        history::oldest_first(hub, &project.id).await?.into_iter().map(|c| (c.snapshot, c.id)).collect();
    let mut mapping = Mapping::load(&target.mapping)?;
    let mut landed = Vec::new();
    for commit in commits {
        let placed = match place(hub, &project, &known, &commit).await {
            Ok(placed) => placed,
            Err(err) => {
                mapping.save()?;
                return Err(err);
            }
        };
        if let Some((id, fresh)) = placed {
            known.insert(commit.snapshot.id(), id.clone());
            mapping.link(id.clone(), commit.commit.to_string());
            if fresh {
                landed.push(id);
            }
        }
    }
    mapping.save()?;
    let bare = repo::open(&target.repo)?;
    if let Some(tip) = repo::target(&bare, &target.fetched_ref())? {
        repo::set(&bare, &target.branch_ref(), tip)?;
    }
    Ok(GitImportOutput { changes: landed })
}

fn fetch_new(store: Arc<dyn Store>, target: &Target) -> Result<Vec<Imported>> {
    let bare = repo::open(&target.repo)?;
    remote::fetch(&target.repo, target.remote()?, &target.branch_ref(), &target.fetched_ref())?;
    let Some(tip) = repo::target(&bare, &target.fetched_ref())? else { return Ok(Vec::new()) };
    let mut mapping = Mapping::load(&target.mapping)?;
    let commits = repo::new_commits(&bare, &mapping, tip)?;
    let imported = commits
        .into_iter()
        .map(|id| repo::read_commit(&bare, store.as_ref(), &mut mapping, id))
        .collect::<Result<Vec<_>>>()?;
    mapping.save()?;
    Ok(imported)
}

async fn place(
    hub: &Hub,
    project: &Project,
    known: &HashMap<SnapshotId, ChangeId>,
    commit: &Imported,
) -> Result<Option<(ChangeId, bool)>> {
    if let Some(id) = known.get(&commit.snapshot.id()) {
        return Ok(Some((id.clone(), false)));
    }
    let files = diff(&commit.parent, &commit.snapshot);
    if files.is_empty() {
        return Ok(None);
    }
    let input = SyncLand {
        project: project.id.clone(),
        parent: commit.parent.id(),
        files,
        author: commit.author.clone(),
        intent: commit.intent.clone(),
        claims: Vec::new(),
    };
    let out = hub.landing.land_change(input).await?;
    match out.change {
        Some(change) if out.landed => Ok(Some((change.id, true))),
        _ => {
            let paths: Vec<String> = out.conflicts.iter().map(|c| c.path.clone()).collect();
            Err(Error::conflict(format!("git commit {} conflicts on: {}", commit.commit, paths.join(", "))))
        }
    }
}

fn diff(before: &Snapshot, after: &Snapshot) -> BTreeMap<String, Option<FileState>> {
    let mut files = BTreeMap::new();
    for entry in &after.entries {
        if before.get(&entry.path) != Some(entry) {
            files.insert(entry.path.clone(), Some(entry.state()));
        }
    }
    for entry in &before.entries {
        if after.get(&entry.path).is_none() {
            files.insert(entry.path.clone(), None);
        }
    }
    files
}
