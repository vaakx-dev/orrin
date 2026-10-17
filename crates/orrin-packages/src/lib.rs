mod area;
mod build;
mod folder;
mod peers;
mod service;
mod source;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Packages, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Packages>> {
    Ok(Arc::new(service::PackageService::new(setup)?))
}
