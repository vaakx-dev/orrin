use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::home::Home;
use crate::id::{PcId, fresh};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub pc: PcId,
    pub name: String,
    pub key: String,
}

impl Identity {
    pub fn load_or_create(home: &Home) -> Result<Self> {
        let path = home.identity();
        if path.exists() {
            return Ok(serde_json::from_slice(&std::fs::read(&path)?)?);
        }
        let identity = Self { pc: PcId::new(), name: machine_name(), key: fresh("key") };
        std::fs::write(&path, serde_json::to_vec_pretty(&identity)?)?;
        Ok(identity)
    }
}

fn machine_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "pc".to_string())
}
