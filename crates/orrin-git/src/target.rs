use std::path::PathBuf;

use orrin_core::model::Project;
use orrin_core::{Error, Home, Result};

#[derive(Clone, Debug)]
pub struct Target {
    pub repo: PathBuf,
    pub mapping: PathBuf,
    pub remote: Option<String>,
    pub branch: String,
}

impl Target {
    pub fn resolve(home: &Home, project: &Project, remote: Option<String>, branch: Option<String>) -> Result<Self> {
        let settings = project.settings.git.as_ref();
        let remote = remote.or_else(|| settings.map(|s| s.remote.clone())).filter(|r| !r.trim().is_empty());
        let branch = branch.or_else(|| settings.map(|s| s.branch.clone())).unwrap_or_else(|| "main".to_string());
        if branch.trim().is_empty() || branch.starts_with('-') {
            return Err(Error::invalid(format!("bad git branch: {branch}")));
        }
        let dir = home.dir("git")?;
        Ok(Self {
            repo: dir.join(format!("{}.git", project.id)),
            mapping: dir.join(format!("{}.json", project.id)),
            remote,
            branch,
        })
    }

    pub fn remote(&self) -> Result<&str> {
        self.remote.as_deref().ok_or_else(|| Error::invalid("no git remote set for this project"))
    }

    pub fn branch_ref(&self) -> String {
        format!("refs/heads/{}", self.branch)
    }

    pub fn fetched_ref(&self) -> String {
        format!("refs/orrin/remote/{}", self.branch)
    }
}
