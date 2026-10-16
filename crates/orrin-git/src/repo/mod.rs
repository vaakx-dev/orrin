mod read;
mod write;

use std::fmt::Display;
use std::path::Path;

use gix::ObjectId;
use gix::refs::transaction::PreviousValue;
use orrin_core::{Error, Result};

pub use read::{Imported, new_commits, read_commit};
pub use write::{write_commit, write_snapshot, write_start};

pub fn open(path: &Path) -> Result<gix::Repository> {
    if path.join("HEAD").exists() { gix::open(path).map_err(fail) } else { gix::init_bare(path).map_err(fail) }
}

pub fn target(repo: &gix::Repository, name: &str) -> Result<Option<ObjectId>> {
    let reference = repo.try_find_reference(name).map_err(fail)?;
    Ok(reference.and_then(|r| r.try_id()).map(|id| id.detach()))
}

pub fn set(repo: &gix::Repository, name: &str, id: ObjectId) -> Result<()> {
    repo.reference(name, id, PreviousValue::Any, "orrin").map_err(fail)?;
    Ok(())
}

pub fn parse(oid: &str) -> Result<ObjectId> {
    ObjectId::from_hex(oid.as_bytes()).map_err(fail)
}

pub fn fail(err: impl Display) -> Error {
    Error::internal(format!("git: {err}"))
}
