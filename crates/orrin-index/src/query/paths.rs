use globset::{Glob, GlobMatcher};
use orrin_core::{Error, Result};

pub enum PathFilter {
    Any,
    Prefix(String),
    Glob(GlobMatcher),
}

impl PathFilter {
    pub fn new(pattern: Option<&str>) -> Result<Self> {
        let Some(pattern) = pattern.map(str::trim).filter(|p| !p.is_empty()) else { return Ok(PathFilter::Any) };
        if pattern.contains(['*', '?', '[', '{']) {
            let glob = Glob::new(pattern).map_err(|e| Error::invalid(e.to_string()))?;
            return Ok(PathFilter::Glob(glob.compile_matcher()));
        }
        Ok(PathFilter::Prefix(pattern.trim_matches('/').to_string()))
    }

    pub fn allows(&self, path: &str) -> bool {
        match self {
            PathFilter::Any => true,
            PathFilter::Prefix(prefix) => {
                path == prefix || path.strip_prefix(prefix.as_str()).is_some_and(|rest| rest.starts_with('/'))
            }
            PathFilter::Glob(glob) => glob.is_match(path),
        }
    }
}
