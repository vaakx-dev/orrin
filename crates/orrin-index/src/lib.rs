mod analyse;
mod engine;
mod outline;
mod par;
mod query;
mod records;
mod service;
mod symbols;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Index, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Index>> {
    Ok(Arc::new(service::IndexService::open(setup)?))
}
