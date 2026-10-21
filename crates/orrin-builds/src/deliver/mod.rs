mod run;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use orrin_core::api::{Event, ViewList};
use orrin_core::model::{Delivery, DeliveryState, ViewKind, ViewState};
use orrin_core::{Error, ProjectId, Result, now_ms};
use tokio::sync::broadcast::error::RecvError;
use tokio::task::AbortHandle;

use crate::context::Context;
use crate::manifest;

#[derive(Default)]
pub struct Deliveries {
    last: Mutex<HashMap<ProjectId, Delivery>>,
    tasks: Mutex<HashMap<ProjectId, AbortHandle>>,
}

impl Deliveries {
    pub fn last(&self, project: &ProjectId) -> Result<Delivery> {
        let last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        last.get(project).cloned().ok_or_else(|| Error::not_found(format!("nothing was delivered for {project} yet")))
    }

    pub fn remove(&self, project: &ProjectId) {
        if let Some(task) = self.tasks.lock().unwrap_or_else(|e| e.into_inner()).remove(project) {
            task.abort();
        }
        self.last.lock().unwrap_or_else(|e| e.into_inner()).remove(project);
    }

    fn set(&self, ctx: &Context, delivery: &Delivery) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        last.insert(delivery.project.clone(), delivery.clone());
        ctx.hub.events.emit(Event::Delivery { delivery: delivery.clone() });
    }

    fn replace(&self, project: &ProjectId, task: AbortHandle) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(old) = tasks.insert(project.clone(), task) {
            old.abort();
        }
    }
}

pub fn follow(ctx: &Arc<Context>) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
    let mut events = ctx.hub.events.subscribe();
    let ctx = ctx.clone();
    runtime.spawn(async move {
        loop {
            match events.recv().await {
                Ok(seen) => {
                    if let Event::MainMoved { project, .. } = seen.event
                        && let Err(err) = start(&ctx, &project, false).await
                    {
                        tracing::warn!("could not deliver the outputs of {project}: {err}");
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub async fn start(ctx: &Arc<Context>, project: &ProjectId, asked: bool) -> Result<Option<Delivery>> {
    let main = ctx.hub.projects.get(project).await?.main;
    let snapshot = ctx.hub.store.snapshot(&main)?;
    let found = manifest::load(&*ctx.hub.store, &snapshot);
    let targets: Vec<String> = match (found, asked) {
        (Ok(manifest), _) => manifest.targets.into_iter().filter(|(_, t)| t.deliver).map(|(name, _)| name).collect(),
        (Err(err), true) => return Err(err),
        (Err(_), false) => Vec::new(),
    };
    let reason = if targets.is_empty() {
        Some(format!("no target in {project} has deliver = true"))
    } else if !has_user_folder(ctx, project).await? {
        Some(format!("{project} has no user folder on this PC"))
    } else {
        None
    };
    if let Some(reason) = reason {
        return if asked { Err(Error::invalid(reason)) } else { Ok(None) };
    }
    let delivery = Delivery {
        project: project.clone(),
        main,
        targets,
        state: DeliveryState::Building,
        jobs: Vec::new(),
        copied: 0,
        unchanged: 0,
        waiting: Vec::new(),
        error: None,
        started_at: now_ms(),
        finished_at: None,
    };
    ctx.deliveries.set(ctx, &delivery);
    let task = tokio::spawn(run::run(ctx.clone(), delivery.clone(), snapshot));
    ctx.deliveries.replace(project, task.abort_handle());
    Ok(Some(delivery))
}

async fn has_user_folder(ctx: &Context, project: &ProjectId) -> Result<bool> {
    let views = ctx.hub.views.list(ViewList { project: Some(project.clone()) }).await?.views;
    Ok(views.iter().any(|v| v.kind == ViewKind::User && v.state == ViewState::Open))
}
