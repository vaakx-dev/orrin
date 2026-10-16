use gix::ObjectId;
use orrin_core::model::{Entry, Mode, Snapshot, clean_path};
use orrin_core::service::Store;
use orrin_core::{Hash, Result};

use crate::mapping::Mapping;
use crate::repo::fail;

pub struct Imported {
    pub commit: ObjectId,
    pub parent: Snapshot,
    pub snapshot: Snapshot,
    pub author: String,
    pub intent: String,
}

pub fn new_commits(repo: &gix::Repository, mapping: &Mapping, tip: ObjectId) -> Result<Vec<ObjectId>> {
    let mut commits = Vec::new();
    let mut next = Some(tip);
    while let Some(id) = next {
        if mapping.change_of(&id.to_string()).is_some() {
            break;
        }
        commits.push(id);
        next = repo.find_commit(id).map_err(fail)?.parent_ids().next().map(|p| p.detach());
    }
    commits.reverse();
    Ok(commits)
}

pub fn read_commit(repo: &gix::Repository, store: &dyn Store, mapping: &mut Mapping, id: ObjectId) -> Result<Imported> {
    let commit = repo.find_commit(id).map_err(fail)?;
    let decoded = commit.decode().map_err(fail)?;
    let author = decoded.author().map_err(fail)?;
    let author = format!("{} <{}>", author.name, author.email);
    let intent = decoded.message.to_string().trim().to_string();
    let parent = match decoded.parents().next() {
        Some(parent) => {
            snapshot(repo, store, mapping, repo.find_commit(parent).map_err(fail)?.tree_id().map_err(fail)?.detach())?
        }
        None => Snapshot::default(),
    };
    let snapshot = snapshot(repo, store, mapping, decoded.tree())?;
    Ok(Imported { commit: id, parent, snapshot, author, intent })
}

fn snapshot(repo: &gix::Repository, store: &dyn Store, mapping: &mut Mapping, tree: ObjectId) -> Result<Snapshot> {
    let mut entries = Vec::new();
    read_tree(repo, store, mapping, tree, "", &mut entries)?;
    let snapshot = Snapshot::new(entries);
    store.put_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn read_tree(
    repo: &gix::Repository,
    store: &dyn Store,
    mapping: &mut Mapping,
    tree: ObjectId,
    prefix: &str,
    out: &mut Vec<Entry>,
) -> Result<()> {
    let tree = repo.find_tree(tree).map_err(fail)?;
    let decoded = tree.decode().map_err(fail)?;
    for entry in &decoded.entries {
        let path = if prefix.is_empty() { entry.filename.to_string() } else { format!("{prefix}/{}", entry.filename) };
        let oid = entry.oid.to_owned();
        if entry.mode.is_tree() {
            read_tree(repo, store, mapping, oid, &path, out)?;
            continue;
        }
        if entry.mode.is_commit() {
            continue;
        }
        let hash = blob(repo, store, mapping, oid)?;
        let mode = if entry.mode.is_executable() { Mode::Executable } else { Mode::File };
        out.push(Entry { path: clean_path(&path)?, size: store.size(&hash)?, hash, mode });
    }
    Ok(())
}

fn blob(repo: &gix::Repository, store: &dyn Store, mapping: &mut Mapping, oid: ObjectId) -> Result<Hash> {
    let key = oid.to_string();
    if let Some(hash) = mapping.object(&key)
        && store.has(hash)
    {
        return Ok(hash.clone());
    }
    let hash = store.put(&repo.find_blob(oid).map_err(fail)?.data)?;
    mapping.remember(hash.clone(), key);
    Ok(hash)
}
