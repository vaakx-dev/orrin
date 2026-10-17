use std::collections::HashSet;
use std::fs::{File, Metadata};
use std::path::Path;

use orrin_core::Result;
use orrin_core::par::par_map;

use crate::paths;
use crate::runner::folder::SCREENSHOTS;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Carried {
    pub copied: usize,
    pub kept: usize,
}

pub fn carry(from: &Path, to: &Path, patterns: &[String], skip: &HashSet<&str>) -> Result<Carried> {
    let globs = paths::globs(patterns)?;
    let paths: Vec<String> = paths::walk(from, patterns)
        .into_iter()
        .filter(|path| globs.is_match(path) && !skip.contains(path.as_str()) && !path.starts_with(SCREENSHOTS))
        .collect();
    let copied = par_map(&paths, |path| copy(&from.join(path), &to.join(path)));
    let mut carried = Carried::default();
    for copied in copied {
        match copied? {
            true => carried.copied += 1,
            false => carried.kept += 1,
        }
    }
    Ok(carried)
}

fn copy(source: &Path, target: &Path) -> Result<bool> {
    let meta = std::fs::metadata(source)?;
    if std::fs::metadata(target).is_ok_and(|have| same(&have, &meta)) {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::remove_file(target) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
        _ => {}
    }
    reflink_copy::reflink_or_copy(source, target)?;
    File::options().write(true).open(target)?.set_modified(meta.modified()?)?;
    Ok(true)
}

fn same(have: &Metadata, want: &Metadata) -> bool {
    have.len() == want.len() && have.modified().ok().is_some_and(|at| want.modified().is_ok_and(|w| w == at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_shared_build_files_with_their_times() {
        let root = tempfile::tempdir().unwrap();
        let (from, to) = (root.path().join("lib"), root.path().join("app"));
        for (path, body) in [("bin/lib.a", "lib"), ("obj/x.o", "x"), ("obj/deep/y.o", "y"), ("src/a.c", "c")] {
            let file = from.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, body).unwrap();
        }
        let patterns = vec!["bin/**".to_string(), "obj/**/*.o".to_string()];
        let skip = HashSet::from(["obj/x.o"]);
        assert_eq!(carry(&from, &to, &patterns, &skip).unwrap(), Carried { copied: 2, kept: 0 });
        let time = |dir: &Path| std::fs::metadata(dir.join("obj/deep/y.o")).unwrap().modified().unwrap();
        assert_eq!(time(&from), time(&to));
        assert!(!to.join("obj/x.o").exists() && !to.join("src").exists());
        assert_eq!(carry(&from, &to, &patterns, &skip).unwrap(), Carried { copied: 0, kept: 2 });
    }
}
