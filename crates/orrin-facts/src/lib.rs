use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Facts, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Facts>> {
    Ok(Arc::new(Unavailable("facts")))
}
