use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Setup, Store, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Store>> {
    Ok(Arc::new(Unavailable("store")))
}
