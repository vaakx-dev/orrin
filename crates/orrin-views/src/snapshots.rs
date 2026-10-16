use std::collections::{BTreeMap, BTreeSet};

use orrin_core::model::{Entry, Snapshot};
use orrin_core::service::Edits;

pub fn with_edits(base: &Snapshot, edits: &Edits) -> Snapshot {
    let mut entries: BTreeMap<String, Entry> = base.entries.iter().map(|e| (e.path.clone(), e.clone())).collect();
    for (path, state) in edits {
        match state {
            Some(s) => entries
                .insert(path.clone(), Entry { path: path.clone(), hash: s.hash.clone(), size: s.size, mode: s.mode }),
            None => entries.remove(path),
        };
    }
    Snapshot::new(entries.into_values().collect())
}

pub fn keeping(next: &Snapshot, old: &Snapshot, paths: &[String]) -> Snapshot {
    let kept: Edits = paths.iter().map(|p| (p.clone(), old.get(p).map(Entry::state))).collect();
    with_edits(next, &kept)
}

pub fn moved_paths(old: &Snapshot, new: &Snapshot) -> BTreeSet<String> {
    let paths = old.entries.iter().chain(&new.entries).map(|e| e.path.as_str());
    paths.filter(|p| old.get(p).map(|e| &e.hash) != new.get(p).map(|e| &e.hash)).map(str::to_string).collect()
}
