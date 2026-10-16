use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Index, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Index>> {
    Ok(Arc::new(Unavailable("index")))
}
