use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::error::{Error, Result};
use crate::home::Home;
use crate::id::PcId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonInfo {
    pub port: u16,
    pub pid: u32,
    pub pc: PcId,
    pub started_at: Millis,
}

impl DaemonInfo {
    pub fn write(&self, home: &Home) -> Result<()> {
        std::fs::write(home.daemon(), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn read(home: &Home) -> Result<Self> {
        let bytes = std::fs::read(home.daemon())
            .map_err(|_| Error::unavailable(format!("no daemon running for {}", home.root().display())))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn remove(home: &Home) {
        let _ = std::fs::remove_file(home.daemon());
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}
