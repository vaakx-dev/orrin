use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::home::Home;
use crate::model::PcSettings;

pub const DEFAULT_PORT: u16 = 4520;
pub const BIG_FILE: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub pc: PcSettings,
    pub history: HistoryConfig,
    pub views: ViewsConfig,
    pub builds: BuildsConfig,
    pub maintenance: MaintenanceConfig,
    pub cache: CacheConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub keep_days: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { keep_days: 30 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MaintenanceConfig {
    pub every_hours: u32,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self { every_hours: 6 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuildsConfig {
    pub cpu: Option<u32>,
    pub ram_mb: Option<u64>,
    pub gpu: u32,
    pub warm_pool: u32,
}

impl Default for BuildsConfig {
    fn default() -> Self {
        Self { cpu: None, ram_mb: None, gpu: 1, warm_pool: 3 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub port: u16,
    pub bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { port: DEFAULT_PORT, bind: "0.0.0.0".to_string() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    pub latest: u32,
    pub daily: u32,
    pub weekly: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { latest: 100, daily: 30, weekly: 26 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewsConfig {
    pub keep_days: u32,
    pub big_file: u64,
}

impl Default for ViewsConfig {
    fn default() -> Self {
        Self { keep_days: 7, big_file: BIG_FILE }
    }
}

impl Config {
    pub fn load(home: &Home) -> Result<Self> {
        match std::fs::read_to_string(home.config()) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_defaults_and_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        assert_eq!(Config::load(&home).unwrap().server.port, DEFAULT_PORT);
        std::fs::write(home.config(), "[server]\nport = 5000\n[pc]\nbuilds = false\n").unwrap();
        let config = Config::load(&home).unwrap();
        assert_eq!(config.server.port, 5000);
        assert!(!config.pc.builds);
        std::fs::write(home.config(), "[server]\nnope = 1\n").unwrap();
        assert!(Config::load(&home).is_err());
    }
}
