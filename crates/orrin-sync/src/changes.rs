use orrin_core::api::{Log, SyncChanges, SyncChangesOutput};
use orrin_core::model::Change;
use orrin_core::service::Hub;
use orrin_core::{ProjectId, Result};

use crate::service::Inner;

const HISTORY_LIMIT: u32 = 100_000;

pub async fn history(hub: &Hub, project: &ProjectId) -> Result<Vec<Change>> {
    let log = Log { project: project.clone(), path: None, before: None, limit: Some(HISTORY_LIMIT) };
    let mut changes = hub.projects.log(log).await?.changes;
    if changes.len() > 1 && changes[0].at > changes[changes.len() - 1].at {
        changes.reverse();
    }
    changes.sort_by_key(|c| c.at);
    Ok(changes)
}

impl Inner {
    pub async fn changes(&self, input: SyncChanges) -> Result<SyncChangesOutput> {
        let project = self.hub.projects.get(&input.project).await?;
        let mut changes = history(&self.hub, &input.project).await?;
        if let Some(index) = input.after.and_then(|after| changes.iter().position(|c| c.id == after)) {
            changes.drain(..=index);
        }
        Ok(SyncChangesOutput { project, changes })
    }
}
