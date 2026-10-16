use std::time::Duration;

use orrin_core::api::{Event, Log, ViewList};
use orrin_core::model::ViewState;
use orrin_core::service::Hub;
use orrin_core::{Error, Hash, ProjectId, Result};

const FIRST_RUN: Duration = Duration::from_secs(2 * 60);
const HOUR: Duration = Duration::from_secs(60 * 60);
const LOG_PAGE: u32 = 500;

pub fn start(hub: &Hub) {
    let hours = hub.config.maintenance.every_hours;
    let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
    if hours == 0 {
        return;
    }
    let hub = hub.clone();
    runtime.spawn(async move {
        tokio::time::sleep(FIRST_RUN).await;
        loop {
            if let Err(err) = run(&hub).await {
                tracing::warn!("maintenance failed: {err}");
            }
            tokio::time::sleep(HOUR * hours).await;
        }
    });
}

pub async fn run(hub: &Hub) -> Result<Event> {
    let mut thinned = 0;
    let mut roots = Vec::new();
    for project in hub.projects.list().await?.projects {
        thinned += hub.projects.thin(&project.id).await?;
        roots.push(project.main);
        roots.extend(history(hub, &project.id).await?);
    }
    roots.extend(views(hub).await?);
    roots.extend(hub.builds.roots().await?);
    let store = hub.store.clone();
    let report = tokio::task::spawn_blocking(move || store.gc(&roots))
        .await
        .map_err(|err| Error::internal(err.to_string()))??;
    let event = Event::Maintenance { thinned, removed: report.removed, freed: report.freed };
    hub.events.emit(event.clone());
    Ok(event)
}

async fn history(hub: &Hub, project: &ProjectId) -> Result<Vec<Hash>> {
    let mut roots = Vec::new();
    let mut before = None;
    loop {
        let log = Log { project: project.clone(), path: None, before, limit: Some(LOG_PAGE) };
        let changes = hub.projects.log(log).await?.changes;
        before = changes.last().map(|change| change.id.clone());
        let full = changes.len() as u32 == LOG_PAGE;
        roots.extend(changes.into_iter().flat_map(|change| [change.snapshot, change.parent]));
        if !full {
            return Ok(roots);
        }
    }
}

async fn views(hub: &Hub) -> Result<Vec<Hash>> {
    let mut roots = Vec::new();
    for view in hub.views.list(ViewList { project: None }).await?.views {
        if view.state == ViewState::Open {
            let edits = hub.views.capture(&view.id).await.unwrap_or_default();
            roots.extend(edits.into_values().flatten().map(|state| state.hash));
        }
        roots.push(view.base);
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use orrin_core::api::ProjectsCreate;
    use orrin_core::{Config, Home};

    use super::*;

    fn age_store(dir: &std::path::Path) {
        let old = SystemTime::now() - Duration::from_secs(24 * 60 * 60);
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                age_store(&entry.path());
            } else {
                std::fs::File::options().write(true).open(entry.path()).unwrap().set_modified(old).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn keeps_mains_and_frees_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        let hub = crate::wire(home.clone(), Config::default()).unwrap();
        let create = ProjectsCreate { name: "demo".into(), home_pc: None, path: None };
        let project = hub.projects.create(create).await.unwrap().project;
        let orphan = hub.store.put(b"nobody points at me").unwrap();
        age_store(&home.store());

        let mut events = hub.events.subscribe();
        let Event::Maintenance { removed, .. } = run(&hub).await.unwrap() else { panic!("wrong event") };
        assert!(removed >= 1);
        assert!(!hub.store.has(&orphan));
        assert!(hub.store.snapshot(&project.main).is_ok());
        while !matches!(events.recv().await.unwrap().event, Event::Maintenance { .. }) {}
    }
}
