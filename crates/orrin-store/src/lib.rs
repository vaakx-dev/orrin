mod big;
mod disk;
mod gc;
mod link;
mod materialise;
mod object;
mod read;
mod seal;
mod service;
mod verified;
mod write;
mod zipped;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Setup, Store};

pub use disk::DiskStore;

pub fn open(setup: &Setup) -> Result<Arc<dyn Store>> {
    Ok(Arc::new(DiskStore::open(setup.home.store())?))
}
