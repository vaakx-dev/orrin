mod keys;
mod peers;
mod service;
mod trim;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Cache, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Cache>> {
    Ok(Arc::new(service::CacheService::open(setup)?))
}
