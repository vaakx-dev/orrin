use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn resolve(over: Option<PathBuf>) -> Result<Self> {
        let root = match over {
            Some(path) => path,
            None => dirs::home_dir().ok_or_else(|| Error::unavailable("no home directory"))?.join(".orrin"),
        };
        Self::at(root)
    }

    pub fn at(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("orrin.toml")
    }

    pub fn daemon(&self) -> PathBuf {
        self.root.join("daemon.json")
    }

    pub fn identity(&self) -> PathBuf {
        self.root.join("identity.json")
    }

    pub fn store(&self) -> PathBuf {
        self.root.join("store")
    }

    pub fn db(&self) -> PathBuf {
        self.root.join("orrin.db")
    }

    pub fn views(&self) -> PathBuf {
        self.root.join("views")
    }

    pub fn runs(&self) -> PathBuf {
        self.root.join("runs")
    }

    pub fn index(&self) -> PathBuf {
        self.root.join("index")
    }

    pub fn dir(&self, name: &str) -> Result<PathBuf> {
        let path = self.root.join(name);
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }
}
