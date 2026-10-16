use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::Hash;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    File,
    Executable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub hash: Hash,
    pub size: u64,
    #[serde(default)]
    pub mode: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileState {
    pub hash: Hash,
    pub size: u64,
    #[serde(default)]
    pub mode: Mode,
}

impl Entry {
    pub fn state(&self) -> FileState {
        FileState { hash: self.hash.clone(), size: self.size, mode: self.mode }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    #[default]
    Utf8,
    Base64,
}

pub fn clean_path(path: &str) -> Result<String> {
    let unified = path.replace('\\', "/");
    let parts: Vec<&str> = unified.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let bad = parts.is_empty() || unified.starts_with('/') || unified.contains(':') || parts.contains(&"..");
    if bad {
        return Err(Error::invalid(format!("bad project path: {path}")));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_paths() {
        assert_eq!(clean_path("a\\b/./c.txt").unwrap(), "a/b/c.txt");
        assert!(clean_path("../x").is_err());
        assert!(clean_path("/abs").is_err());
        assert!(clean_path("C:/x").is_err());
    }
}
