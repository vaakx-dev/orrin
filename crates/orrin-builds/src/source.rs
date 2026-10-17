use std::collections::BTreeMap;

use orrin_core::api::JobInput;
use orrin_core::model::{Entry, Snapshot};
use orrin_core::service::Services;
use orrin_core::{Error, Result, SnapshotId, ViewId};

pub async fn resolve(hub: &Services, input: &JobInput) -> Result<(SnapshotId, Snapshot)> {
    match (&input.view, &input.snapshot) {
        (Some(_), Some(_)) => Err(Error::invalid("give a view or a snapshot, not both")),
        (Some(view), None) => from_view(hub, input, view).await,
        (None, Some(id)) => Ok((id.clone(), hub.store.snapshot(id)?)),
        (None, None) => {
            let project = hub.projects.get(&input.project).await?;
            let snapshot = hub.store.snapshot(&project.main)?;
            Ok((project.main, snapshot))
        }
    }
}

async fn from_view(hub: &Services, input: &JobInput, id: &ViewId) -> Result<(SnapshotId, Snapshot)> {
    let view = hub.views.get(id).await?;
    if view.project != input.project {
        return Err(Error::invalid(format!("view {id} is not in project {}", input.project)));
    }
    let edits = hub.views.capture(id).await?;
    let base = hub.store.snapshot(&view.base)?;
    let mut files: BTreeMap<String, Entry> = base.entries.into_iter().map(|e| (e.path.clone(), e)).collect();
    for (path, edit) in edits {
        match edit {
            None => {
                files.remove(&path);
            }
            Some(state) => {
                let hash = match hub.store.has(&state.hash) {
                    true => state.hash,
                    false => hub.store.put_file(&view.path.join(&path))?,
                };
                files.insert(path.clone(), Entry { path, hash, size: state.size, mode: state.mode });
            }
        }
    }
    let snapshot = Snapshot::new(files.into_values().collect());
    let id = hub.store.put_snapshot(&snapshot)?;
    Ok((id, snapshot))
}
