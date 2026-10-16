mod book;
mod probe;
mod scan;
mod service;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Facts, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Facts>> {
    Ok(Arc::new(service::FactsService::open(setup)?))
}
