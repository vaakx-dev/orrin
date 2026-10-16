use orrin_core::api::{Event, LandOutput, ViewList};
use orrin_core::model::{View, ViewKind, ViewState};
use orrin_core::service::Services;
use orrin_core::{ProjectId, SnapshotId};

pub async fn settle(hub: &Services, project: &ProjectId, landed: Option<&View>, mut out: LandOutput) -> LandOutput {
    if let Some(view) = landed
        && !out.conflicts.is_empty()
    {
        hub.events.emit(Event::ViewConflicts { view: view.id.clone(), conflicts: out.conflicts.clone() });
    }
    let Some(change) = &out.change else {
        return out;
    };
    if let Some(view) = landed {
        if let Err(err) = hub.views.landed(&view.id, &change.id).await {
            tracing::warn!(view = %view.id, "could not mark view landed: {err}");
        }
        hub.events.emit(Event::ViewLanded { view: view.id.clone(), change: change.id.clone() });
    }
    out.skipped = advance(hub, project, landed, &change.snapshot).await;
    out
}

async fn advance(hub: &Services, project: &ProjectId, landed: Option<&View>, main: &SnapshotId) -> Vec<String> {
    let views = match hub.views.list(ViewList { project: Some(project.clone()) }).await {
        Ok(list) => list.views,
        Err(err) => {
            tracing::warn!(project = %project, "could not list views to advance: {err}");
            return Vec::new();
        }
    };
    let mut skipped = Vec::new();
    for view in views.iter().filter(|view| follows(view, landed)) {
        match hub.views.advance(&view.id, main).await {
            Ok(paths) if paths.is_empty() => {}
            Ok(paths) => {
                if view.kind == ViewKind::User {
                    skipped.extend(paths.iter().cloned());
                }
                hub.events.emit(Event::ViewSkipped { view: view.id.clone(), paths });
            }
            Err(err) => tracing::warn!(view = %view.id, "could not advance view: {err}"),
        }
    }
    skipped
}

fn follows(view: &View, landed: Option<&View>) -> bool {
    if landed.is_some_and(|landed| landed.id == view.id) {
        return view.kind == ViewKind::User;
    }
    view.state == ViewState::Open
}
