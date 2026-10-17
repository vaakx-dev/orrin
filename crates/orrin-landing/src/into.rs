use orrin_core::api::{Event, LandOutput};
use orrin_core::model::{Conflict, Snapshot, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Hub};
use orrin_core::{Error, Result, ViewId};

use crate::proposal::Proposal;
use crate::rebase::{apply, rebase};

pub async fn land_into(hub: &Hub, view: &View, files: Edits, intent: String, into: &ViewId) -> Result<LandOutput> {
    let parent = hub.views.get(into).await?;
    check(view, &parent)?;
    let current = apply(&hub.store.snapshot(&parent.base)?, &hub.views.capture(into).await?);
    let expected = hub.store.put_snapshot(&current)?;
    let proposal = Proposal::from_view(view, files, intent);
    let mut rebased = rebase(hub, &hub.store.snapshot(&view.base)?, &current, &proposal)?;
    if rebased.conflicts.is_empty() && !rebased.files.is_empty() {
        let missed = hub.views.take(into, &expected, &rebased.files).await?;
        rebased.conflicts = missed.into_iter().map(|path| moved_meanwhile(&rebased.files, &current, path)).collect();
    }
    let main = hub.projects.get(&view.project).await?.main;
    let landed = rebased.conflicts.is_empty() && !rebased.files.is_empty();
    if landed {
        hub.views.landed(&view.id, None).await?;
        hub.events.emit(Event::ViewLanded { view: view.id.clone(), change: None, into: Some(into.clone()) });
    } else if !rebased.conflicts.is_empty() {
        hub.events.emit(Event::ViewConflicts { view: view.id.clone(), conflicts: rebased.conflicts.clone() });
    }
    Ok(LandOutput { landed, change: None, main, conflicts: rebased.conflicts, skipped: Vec::new() })
}

fn check(view: &View, parent: &View) -> Result<()> {
    if view.kind == ViewKind::User {
        return Err(Error::invalid(format!("user view {} can only land to main", view.id)));
    }
    if parent.id == view.id || parent.project != view.project || parent.state != ViewState::Open {
        return Err(Error::invalid(format!("view {} is not an open view of project {}", parent.id, view.project)));
    }
    Ok(())
}

fn moved_meanwhile(files: &Edits, current: &Snapshot, path: String) -> Conflict {
    Conflict {
        base: None,
        ours: files.get(&path).cloned().flatten().map(|s| s.hash),
        theirs: current.get(&path).map(|e| e.hash.clone()),
        reason: "changed in the target view while landing, or open; land again".into(),
        path,
    }
}
