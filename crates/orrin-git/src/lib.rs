mod export;
mod history;
mod import;
mod mapping;
mod remote;
mod repo;
mod service;
mod target;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Git, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Git>> {
    Ok(Arc::new(service::GitService::new(setup)))
}
