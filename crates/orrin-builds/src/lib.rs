mod cache;
mod context;
mod deliver;
mod journal;
mod key;
mod linked;
mod manifest;
mod paths;
mod plan;
mod process;
mod queue;
mod recover;
mod remote;
mod roots;
mod runner;
mod service;
mod source;
mod table;
mod tools;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Builds, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Builds>> {
    Ok(Arc::new(service::BuildService::new(setup)?))
}
