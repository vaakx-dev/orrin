use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Peers, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Peers>> {
    Ok(Arc::new(Unavailable("sync")))
}
