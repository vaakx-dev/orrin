use orrin_core::api::{PullOutput, ViewPull};
use orrin_core::model::{Conflict, FileState, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Hub};
use orrin_core::{Error, Result, SnapshotId, ViewId};

use crate::claims::Claims;
use crate::rebase::{Resolved, apply, hash, resolve, state};

pub async fn pull(hub: &Hub, input: ViewPull) -> Result<PullOutput> {
    let view = hub.views.get(&input.view).await?;
    if view.kind != ViewKind::Worker || view.state != ViewState::Open {
        return Err(Error::invalid(format!("view {} is not an open worker view", view.id)));
    }
    let source = match &input.from {
        Some(from) => current(hub, &view, from).await?,
        None => hub.projects.get(&view.project).await?.main,
    };
    let (base, theirs) = (hub.store.snapshot(&view.base)?, hub.store.snapshot(&source)?);
    let claims = Claims::new(&[])?;
    let mut merged = Edits::new();
    let mut conflicts = Vec::new();
    for (path, ours) in &hub.views.capture(&view.id).await? {
        let (old, new) = (state(&base, path), state(&theirs, path));
        if new == old || new == *ours {
            continue;
        }
        let clash = |reason: String| Conflict {
            path: path.clone(),
            base: hash(&old),
            ours: hash(ours),
            theirs: hash(&new),
            reason,
        };
        match resolve(hub, &claims, path, &old, ours, &new)? {
            Resolved::Keep => {}
            Resolved::Take(result) => _ = merged.insert(path.clone(), result),
            Resolved::Conflict(reason) => match marked(hub, path, &old, ours, &new)? {
                Some(result) => {
                    merged.insert(path.clone(), Some(result));
                    conflicts.push(clash(format!("{reason}; conflict markers are in the file, fix them and land")));
                }
                None => conflicts.push(clash(format!("{reason}; kept your version"))),
            },
        }
    }
    let pulled = hub.views.pull(&view.id, &source, &merged).await?;
    for path in pulled.skipped {
        if !conflicts.iter().any(|c| c.path == path) {
            let reason = "changed in your view while pulling; pull again".to_string();
            conflicts.push(Conflict { path, base: None, ours: None, theirs: None, reason });
        }
    }
    Ok(PullOutput { changed: pulled.written, conflicts })
}

async fn current(hub: &Hub, view: &View, from: &ViewId) -> Result<SnapshotId> {
    let parent = hub.views.get(from).await?;
    if parent.id == view.id || parent.project != view.project || parent.state != ViewState::Open {
        return Err(Error::invalid(format!("view {from} is not an open view of project {}", view.project)));
    }
    let edits = hub.views.capture(from).await?;
    hub.store.put_snapshot(&apply(&hub.store.snapshot(&parent.base)?, &edits))
}

fn marked(
    hub: &Hub,
    path: &str,
    base: &Option<FileState>,
    ours: &Option<FileState>,
    theirs: &Option<FileState>,
) -> Result<Option<FileState>> {
    let (Some(ours), Some(theirs), Some(merger)) = (ours, theirs, hub.merger_for(path)) else {
        return Ok(None);
    };
    let base = match base {
        Some(base) => hub.store.get(&base.hash)?,
        None => Vec::new(),
    };
    let Some(bytes) = merger.markers(&base, &hub.store.get(&ours.hash)?, &hub.store.get(&theirs.hash)?) else {
        return Ok(None);
    };
    let hash = hub.store.put(&bytes)?;
    Ok(Some(FileState { hash, size: bytes.len() as u64, mode: ours.mode }))
}
