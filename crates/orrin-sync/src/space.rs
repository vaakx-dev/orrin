use std::path::Path;

use orrin_core::model::PcSettings;
use orrin_core::{Error, Result};

const GB: u64 = 1 << 30;

pub struct Space {
    limit: Option<u64>,
    used: Option<u64>,
}

impl Space {
    pub fn new(settings: &PcSettings) -> Self {
        Self { limit: settings.space_gb.map(|gb| gb * GB), used: None }
    }

    pub fn check(&mut self, store: &Path) -> Result<()> {
        let Some(limit) = self.limit else { return Ok(()) };
        let used = *self.used.get_or_insert_with(|| used_bytes(store));
        if used >= limit {
            return Err(Error::unavailable(format!("store uses {used} bytes, over the {limit} byte space limit")));
        }
        Ok(())
    }
}

fn used_bytes(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
