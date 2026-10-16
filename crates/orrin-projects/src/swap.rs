use orrin_core::model::{Change, Project};
use orrin_core::{Error, Result, SnapshotId};
use rusqlite::{Connection, TransactionBehavior};

use crate::{change_rows, project_rows};

pub enum Swap {
    Moved(Project),
    Missing,
    Stale(SnapshotId),
    Duplicate,
}

pub fn swap_main(conn: &mut Connection, change: &Change) -> rusqlite::Result<Swap> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(mut project) = project_rows::get(&tx, &change.project)? else {
        return Ok(Swap::Missing);
    };
    if project.main != change.parent {
        return Ok(Swap::Stale(project.main));
    }
    if !change_rows::insert(&tx, change)? {
        return Ok(Swap::Duplicate);
    }
    project.main = change.snapshot.clone();
    project_rows::update(&tx, &project)?;
    tx.commit()?;
    Ok(Swap::Moved(project))
}

impl Swap {
    pub fn into_result(self, change: &Change) -> Result<Project> {
        match self {
            Swap::Moved(project) => Ok(project),
            Swap::Missing => Err(Error::not_found(format!("no project {}", change.project))),
            Swap::Stale(main) => Err(Error::conflict(format!("main moved: expected {}, found {main}", change.parent))),
            Swap::Duplicate => Err(Error::conflict(format!("change {} already exists", change.id))),
        }
    }
}
