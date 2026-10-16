use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Git, Setup, Unavailable};

pub fn open(_setup: &Setup) -> Result<Arc<dyn Git>> {
    Ok(Arc::new(Unavailable("git")))
}
