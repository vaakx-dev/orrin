mod catalog;
mod change_rows;
mod db;
mod folder;
mod project_rows;
mod swap;
mod thin;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Projects, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Projects>> {
    Ok(Arc::new(catalog::Catalog::open(setup)?))
}
