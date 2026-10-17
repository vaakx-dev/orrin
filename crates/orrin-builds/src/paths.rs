use std::collections::BTreeSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use globset::{Glob, GlobSet, GlobSetBuilder};
use orrin_core::{Error, Millis, Result};
use walkdir::WalkDir;

pub fn globs(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| Error::invalid(format!("bad glob {pattern}: {err}")))?;
        builder.add(glob);
    }
    builder.build().map_err(|err| Error::invalid(err.to_string()))
}

pub fn modified(meta: &Metadata) -> Option<Millis> {
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(since.as_millis() as Millis)
}

pub fn walk(dir: &Path, patterns: &[String]) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for base in roots(patterns) {
        let files = WalkDir::new(dir.join(base)).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file());
        for entry in files {
            if let Ok(relative) = entry.path().strip_prefix(dir) {
                paths.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    paths
}

fn roots(patterns: &[String]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = patterns
        .iter()
        .map(|pattern| {
            let parts: Vec<&str> = pattern.split('/').collect();
            let dirs = &parts[..parts.len().saturating_sub(1)];
            let fixed = dirs.iter().take_while(|part| !part.contains(['*', '?', '[', '{']));
            fixed.copied().collect::<Vec<_>>().join("/").into()
        })
        .collect();
    roots.sort();
    roots.dedup_by(|later, earlier| later.starts_with(&*earlier));
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_only_below_the_fixed_part_of_each_glob() {
        let patterns = ["bin/**", "obj/**/*.o", "obj/x/*.a", "*.txt"].map(String::from);
        assert_eq!(roots(&patterns[..3]), [PathBuf::from("bin"), PathBuf::from("obj")]);
        assert_eq!(roots(&patterns), [PathBuf::new()]);
    }
}
