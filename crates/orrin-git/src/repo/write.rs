use std::collections::BTreeMap;

use gix::ObjectId;
use gix::actor::Signature;
use gix::date::Time;
use gix::objs::tree::{Entry, EntryKind};
use gix::objs::{Commit, Tree};
use orrin_core::Result;
use orrin_core::model::{Change, Mode, Snapshot};
use orrin_core::service::Store;

use crate::mapping::Mapping;
use crate::repo::{fail, parse};

enum Node {
    File(ObjectId, EntryKind),
    Dir(BTreeMap<String, Node>),
}

pub fn write_snapshot(
    repo: &gix::Repository,
    store: &dyn Store,
    mapping: &mut Mapping,
    snapshot: &Snapshot,
) -> Result<ObjectId> {
    let mut root = BTreeMap::new();
    for entry in &snapshot.entries {
        let oid = blob(repo, store, mapping, &entry.hash)?;
        let kind = if entry.mode == Mode::Executable { EntryKind::BlobExecutable } else { EntryKind::Blob };
        insert(&mut root, &entry.path, Node::File(oid, kind));
    }
    write_dir(repo, &root)
}

pub fn write_commit(
    repo: &gix::Repository,
    tree: ObjectId,
    parent: Option<ObjectId>,
    change: &Change,
) -> Result<ObjectId> {
    let intent = if change.intent.trim().is_empty() { "orrin change" } else { change.intent.trim() };
    let message = format!("{intent}\n\nOrrin-Change: {}\n", change.id);
    commit(repo, tree, parent, signature(&change.author, change.at), message)
}

pub fn write_start(repo: &gix::Repository, tree: ObjectId, before: &Change) -> Result<ObjectId> {
    let message = format!("Starting point\n\nOrrin-Snapshot: {}\n", before.parent);
    commit(repo, tree, None, signature("orrin", before.at), message)
}

fn commit(
    repo: &gix::Repository,
    tree: ObjectId,
    parent: Option<ObjectId>,
    signature: Signature,
    message: String,
) -> Result<ObjectId> {
    let commit = Commit {
        tree,
        parents: parent.into_iter().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: message.into(),
        extra_headers: Vec::new(),
    };
    Ok(repo.write_object(commit).map_err(fail)?.detach())
}

fn blob(repo: &gix::Repository, store: &dyn Store, mapping: &mut Mapping, hash: &orrin_core::Hash) -> Result<ObjectId> {
    if let Some(oid) = mapping.blob(hash).map(parse).transpose()?
        && repo.has_object(oid)
    {
        return Ok(oid);
    }
    let oid = repo.write_blob(store.get(hash)?).map_err(fail)?.detach();
    mapping.remember(hash.clone(), oid.to_string());
    Ok(oid)
}

fn insert(dir: &mut BTreeMap<String, Node>, path: &str, leaf: Node) {
    match path.split_once('/') {
        None => {
            dir.insert(path.to_string(), leaf);
        }
        Some((head, rest)) => {
            let node = dir.entry(head.to_string()).or_insert_with(|| Node::Dir(BTreeMap::new()));
            if let Node::File(..) = node {
                *node = Node::Dir(BTreeMap::new());
            }
            if let Node::Dir(sub) = node {
                insert(sub, rest, leaf);
            }
        }
    }
}

fn write_dir(repo: &gix::Repository, dir: &BTreeMap<String, Node>) -> Result<ObjectId> {
    let mut entries = Vec::with_capacity(dir.len());
    for (name, node) in dir {
        let (kind, oid) = match node {
            Node::File(oid, kind) => (*kind, *oid),
            Node::Dir(sub) => (EntryKind::Tree, write_dir(repo, sub)?),
        };
        entries.push(Entry { mode: kind.into(), filename: name.as_str().into(), oid });
    }
    entries.sort();
    Ok(repo.write_object(Tree { entries }).map_err(fail)?.detach())
}

fn signature(author: &str, at: u64) -> Signature {
    let (name, email) = match author.split_once('<') {
        Some((name, rest)) => (name.trim().to_string(), rest.trim_end_matches('>').trim().to_string()),
        None => (author.trim().to_string(), format!("{}@orrin", author.trim().replace(' ', "."))),
    };
    let name = if name.is_empty() { "orrin".to_string() } else { name };
    Signature { name: name.into(), email: email.into(), time: Time::new((at / 1000) as i64, 0) }
}
