use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Setup, Unavailable, Views};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Views>> {
    Ok(Arc::new(Unavailable("views")))
}
