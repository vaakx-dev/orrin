use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Projects, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Projects>> {
    Ok(Arc::new(Unavailable("projects")))
}
