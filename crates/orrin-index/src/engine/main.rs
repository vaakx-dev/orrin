use std::collections::HashSet;
use std::sync::Arc;

use orrin_core::model::Snapshot;
use orrin_core::{ErrorCode, ProjectId, Result, SnapshotId};

use super::content::{Item, blocking};
use super::{Engine, ProjectIndex};

impl Engine {
    pub(super) async fn sync_main(&self, project: &ProjectId) -> Result<Arc<ProjectIndex>> {
        let main = self.hub.projects.get(project).await?.main;
        let index = self.index(project);
        if !index.at(&main) {
            self.refresh_main(project, &index, main).await?;
        }
        Ok(index)
    }

    async fn refresh_main(&self, project: &ProjectId, index: &ProjectIndex, main: SnapshotId) -> Result<()> {
        let _gate = index.gate.lock().await;
        if index.at(&main) {
            return Ok(());
        }
        let snapshot = self.load_snapshot(&main).await?;
        let (changed, removed) = {
            let known = index.read().main.hashes();
            let changed: Vec<Item> = snapshot
                .entries
                .iter()
                .filter(|e| known.get(&e.path) != Some(&e.hash))
                .map(|e| Item { path: e.path.clone(), hash: e.hash.clone(), size: e.size, bytes: None })
                .collect();
            let present: HashSet<&str> = snapshot.entries.iter().map(|e| e.path.as_str()).collect();
            let removed: Vec<String> = known.into_keys().filter(|p| !present.contains(p.as_str())).collect();
            (changed, removed)
        };
        let resolved = self.resolve(changed, None).await?;
        let clock = self.tick()?;
        {
            let mut state = index.write();
            for path in &removed {
                state.main.remove(path);
            }
            for (item, record) in resolved {
                state.main.insert(item.path, item.hash, item.size, record);
            }
            state.snapshot = Some(main);
            state.clock = clock;
        }
        self.announce(project, None, clock);
        Ok(())
    }

    async fn load_snapshot(&self, id: &SnapshotId) -> Result<Snapshot> {
        let store = self.hub.store.clone();
        let wanted = id.clone();
        match blocking(move || store.snapshot(&wanted)).await? {
            Err(err) if err.code == ErrorCode::NotFound && *id == Snapshot::default().id() => Ok(Snapshot::default()),
            other => other,
        }
    }
}
