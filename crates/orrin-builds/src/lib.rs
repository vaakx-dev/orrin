use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Builds, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Builds>> {
    Ok(Arc::new(Unavailable("builds")))
}
