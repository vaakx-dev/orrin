use orrin_core::api::{Event, LandOutput, ViewList};
use orrin_core::model::{View, ViewKind, ViewState};
use orrin_core::service::Hub;
use orrin_core::{ProjectId, SnapshotId};
use tokio::task::JoinSet;

pub async fn settle(hub: &Hub, project: &ProjectId, landed: Option<&View>, mut out: LandOutput) -> LandOutput {
    if let Some(view) = landed
        && !out.conflicts.is_empty()
    {
        hub.events.emit(Event::ViewConflicts { view: view.id.clone(), conflicts: out.conflicts.clone() });
    }
    let Some(change) = &out.change else {
        return out;
    };
    if let Some(view) = landed {
        if let Err(err) = hub.views.landed(&view.id, Some(&change.id)).await {
            tracing::warn!(view = %view.id, "could not mark view landed: {err}");
        }
        hub.events.emit(Event::ViewLanded { view: view.id.clone(), change: Some(change.id.clone()), into: None });
    }
    out.skipped = advance(hub, project, landed, &change.snapshot).await;
    out
}

async fn advance(hub: &Hub, project: &ProjectId, landed: Option<&View>, main: &SnapshotId) -> Vec<String> {
    let views = match hub.views.list(ViewList { project: Some(project.clone()) }).await {
        Ok(list) => list.views,
        Err(err) => {
            tracing::warn!(project = %project, "could not list views to advance: {err}");
            return Vec::new();
        }
    };
    let mut moving = JoinSet::new();
    for view in views.into_iter().filter(|view| follows(view, landed)) {
        let (hub, main) = (hub.clone(), main.clone());
        moving.spawn(async move { (hub.views.advance(&view.id, &main).await, view) });
    }
    let mut skipped = Vec::new();
    while let Some(done) = moving.join_next().await {
        match done {
            Ok((Ok(paths), view)) if view.kind == ViewKind::User => skipped.extend(paths),
            Ok((Ok(_), _)) => {}
            Ok((Err(err), view)) => tracing::warn!(view = %view.id, "could not advance view: {err}"),
            Err(err) => tracing::warn!(project = %project, "advancing a view stopped: {err}"),
        }
    }
    skipped.sort();
    skipped
}

fn follows(view: &View, landed: Option<&View>) -> bool {
    if landed.is_some_and(|landed| landed.id == view.id) {
        return view.kind == ViewKind::User;
    }
    view.state == ViewState::Open && view.base_view.is_none()
}
