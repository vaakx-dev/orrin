use std::collections::HashSet;

use orrin_core::api::Log;
use orrin_core::model::Change;
use orrin_core::service::Hub;
use orrin_core::{ProjectId, Result};

const PAGE: u32 = 500;

pub async fn oldest_first(hub: &Hub, project: &ProjectId) -> Result<Vec<Change>> {
    let mut changes: Vec<Change> = Vec::new();
    let mut seen = HashSet::new();
    let mut before = None;
    loop {
        let input = Log { project: project.clone(), path: None, before, limit: Some(PAGE) };
        let page = hub.projects.log(input).await?.changes;
        let full = page.len() as u32 >= PAGE;
        let fresh: Vec<Change> = page.into_iter().filter(|c| seen.insert(c.id.clone())).collect();
        let Some(last) = fresh.last() else { break };
        before = Some(last.id.clone());
        changes.extend(fresh);
        if !full {
            break;
        }
    }
    changes.reverse();
    changes.sort_by_key(|c| c.at);
    Ok(changes)
}
