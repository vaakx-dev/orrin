use std::collections::HashSet;

use orrin_core::api::{Log, Undo};
use orrin_core::model::{Change, Entry};
use orrin_core::service::Services;
use orrin_core::{Error, Result};

use crate::proposal::Proposal;

pub async fn proposal(hub: &Services, input: &Undo) -> Result<Proposal> {
    let change = pick(hub, input).await?;
    let before = hub.store.snapshot(&change.parent)?;
    let files = change.files.keys().map(|path| (path.clone(), before.get(path).map(Entry::state))).collect();
    Ok(Proposal {
        project: change.project.clone(),
        parent: change.snapshot.clone(),
        files,
        author: input.author.clone().unwrap_or_else(|| change.author.clone()),
        intent: format!("undo: {}", change.intent),
        claims: Vec::new(),
        undoes: Some(change.id),
    })
}

async fn pick(hub: &Services, input: &Undo) -> Result<Change> {
    if let Some(id) = &input.change {
        let change = hub.projects.change(id).await?;
        if change.project != input.project {
            return Err(Error::invalid(format!("change {id} is not in project {}", input.project)));
        }
        return Ok(change);
    }
    let log = Log { project: input.project.clone(), path: None, before: None, limit: None };
    let changes = hub.projects.log(log).await?.changes;
    let undone: HashSet<_> = changes.iter().filter_map(|c| c.undoes.clone()).collect();
    changes
        .into_iter()
        .filter(|c| c.undoes.is_none() && !undone.contains(&c.id))
        .filter(|c| input.author.as_ref().is_none_or(|author| *author == c.author))
        .max_by_key(|c| c.at)
        .ok_or_else(|| Error::not_found("no landed change to undo"))
}
