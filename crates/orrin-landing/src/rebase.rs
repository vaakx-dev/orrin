use orrin_core::Result;
use orrin_core::model::{Conflict, Entry, FileState, Snapshot};
use orrin_core::service::{Edits, Merged, Services};

use crate::claims::Claims;
use crate::proposal::Proposal;

#[derive(Default)]
pub struct Rebased {
    pub files: Edits,
    pub conflicts: Vec<Conflict>,
}

enum Resolved {
    Keep,
    Take(Option<FileState>),
    Conflict(String),
}

pub fn rebase(hub: &Services, base: &Snapshot, main: &Snapshot, proposal: &Proposal) -> Result<Rebased> {
    let claims = Claims::new(&proposal.claims)?;
    let mut out = Rebased::default();
    for (path, ours) in &proposal.files {
        let base = state(base, path);
        let theirs = state(main, path);
        match resolve(hub, &claims, path, &base, ours, &theirs)? {
            Resolved::Keep => {}
            Resolved::Take(result) if result == theirs => {}
            Resolved::Take(result) => {
                out.files.insert(path.clone(), result);
            }
            Resolved::Conflict(reason) => out.conflicts.push(Conflict {
                path: path.clone(),
                base: hash(&base),
                ours: hash(ours),
                theirs: hash(&theirs),
                reason,
            }),
        }
    }
    Ok(out)
}

pub fn apply(main: &Snapshot, files: &Edits) -> Snapshot {
    let mut entries: Vec<Entry> = main.entries.iter().filter(|e| !files.contains_key(&e.path)).cloned().collect();
    for (path, state) in files {
        if let Some(state) = state {
            entries.push(Entry { path: path.clone(), hash: state.hash.clone(), size: state.size, mode: state.mode });
        }
    }
    Snapshot::new(entries)
}

fn resolve(
    hub: &Services,
    claims: &Claims,
    path: &str,
    base: &Option<FileState>,
    ours: &Option<FileState>,
    theirs: &Option<FileState>,
) -> Result<Resolved> {
    if ours == theirs || ours == base {
        return Ok(Resolved::Keep);
    }
    if theirs == base || claims.covers(path) {
        return Ok(Resolved::Take(ours.clone()));
    }
    let (Some(ours), Some(theirs)) = (ours, theirs) else {
        return Ok(Resolved::Conflict("edited on one side and deleted on the other".into()));
    };
    let Some(merger) = hub.merger_for(path) else {
        return Ok(Resolved::Conflict("both sides changed a file that can't be merged; claim it to overwrite".into()));
    };
    let base = match base {
        Some(base) => hub.store.get(&base.hash)?,
        None => Vec::new(),
    };
    let merged = merger.merge(path, &base, &hub.store.get(&ours.hash)?, &hub.store.get(&theirs.hash)?);
    Ok(match merged {
        Merged::Clean(bytes) => {
            let hash = hub.store.put(&bytes)?;
            Resolved::Take(Some(FileState { hash, size: bytes.len() as u64, mode: ours.mode }))
        }
        Merged::Conflict(reason) => Resolved::Conflict(reason),
    })
}

fn state(snapshot: &Snapshot, path: &str) -> Option<FileState> {
    snapshot.get(path).map(Entry::state)
}

fn hash(state: &Option<FileState>) -> Option<orrin_core::Hash> {
    state.as_ref().map(|s| s.hash.clone())
}
