mod content;
mod disk;
mod inner;
mod ops;
mod pools;
mod service;
mod slot;
mod snapshots;
mod spare;
mod watch;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Setup, Views};

pub fn open(setup: &Setup) -> Result<Arc<dyn Views>> {
    Ok(Arc::new(service::ViewsService(Arc::new(inner::Inner::new(setup)?))))
}
