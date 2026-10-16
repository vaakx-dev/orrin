use orrin_core::api::LandOutput;
use orrin_core::model::{Change, Conflict};
use orrin_core::service::Services;
use orrin_core::{ChangeId, Error, ErrorCode, Result, SnapshotId, now_ms};

use crate::proposal::Proposal;
use crate::rebase::{apply, rebase};

const ATTEMPTS: usize = 16;

pub async fn commit(hub: &Services, proposal: &Proposal) -> Result<LandOutput> {
    let base = hub.store.snapshot(&proposal.parent)?;
    for _ in 0..ATTEMPTS {
        let project = hub.projects.get(&proposal.project).await?;
        let main = hub.store.snapshot(&project.main)?;
        let rebased = rebase(hub, &base, &main, proposal)?;
        if !rebased.conflicts.is_empty() || rebased.files.is_empty() {
            return Ok(output(project.main, None, rebased.conflicts));
        }
        let snapshot = hub.store.put_snapshot(&apply(&main, &rebased.files))?;
        let change = Change {
            id: ChangeId::new(),
            project: project.id.clone(),
            parent: project.main.clone(),
            snapshot: snapshot.clone(),
            files: rebased.files,
            author: proposal.author.clone(),
            intent: proposal.intent.clone(),
            at: now_ms(),
            pinned: false,
            undoes: proposal.undoes.clone(),
        };
        match hub.projects.swap_main(change.clone()).await {
            Ok(_) => {
                hub.projects.append(vec![change.clone()]).await?;
                return Ok(output(snapshot, Some(change), Vec::new()));
            }
            Err(err) if err.code == ErrorCode::Conflict => continue,
            Err(err) => return Err(err),
        }
    }
    Err(Error::conflict("main kept moving while landing; try again"))
}

fn output(main: SnapshotId, change: Option<Change>, conflicts: Vec<Conflict>) -> LandOutput {
    LandOutput { landed: change.is_some(), change, main, conflicts, skipped: Vec::new() }
}
