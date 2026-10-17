use std::fs::{self, DirEntry, Metadata};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rayon::Scope;

const IGNORE_FILES: [&str; 2] = [".gitignore", ".ignore"];

pub type Keep<'a> = dyn Fn(&Path, bool, &Nested) -> bool + Sync + 'a;

#[derive(Default)]
pub struct Nested(Vec<Arc<Gitignore>>);

struct Walk<'a> {
    nested: bool,
    keep: &'a Keep<'a>,
    found: Mutex<Vec<(PathBuf, Metadata)>>,
}

pub fn files(start: &Path, nested: bool, keep: &Keep) -> Vec<(PathBuf, Metadata)> {
    let walk = Walk { nested, keep, found: Mutex::default() };
    rayon::scope(|scope| walk.visit(scope, start.to_path_buf(), Arc::default()));
    walk.found.into_inner().unwrap_or_else(|e| e.into_inner())
}

impl<'a> Walk<'a> {
    fn visit<'s>(&'s self, scope: &Scope<'s>, dir: PathBuf, chain: Arc<Nested>) {
        let Ok(listing) = fs::read_dir(&dir) else { return };
        let entries: Vec<DirEntry> = listing.flatten().collect();
        let chain = if self.nested { chain.within(&dir, &entries) } else { chain };
        let mut files = Vec::new();
        for entry in entries {
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_symlink() || !(self.keep)(&path, kind.is_dir(), &chain) {
                continue;
            }
            if kind.is_dir() {
                let chain = chain.clone();
                scope.spawn(move |scope| self.visit(scope, path, chain));
            } else if let Some(meta) = entry.metadata().ok().filter(Metadata::is_file) {
                files.push((path, meta));
            }
        }
        self.found.lock().unwrap_or_else(|e| e.into_inner()).extend(files);
    }
}

impl Nested {
    pub fn ignores(&self, abs: &Path, is_dir: bool) -> bool {
        for rules in self.0.iter().rev() {
            match rules.matched(abs, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }

    fn within(self: &Arc<Self>, dir: &Path, entries: &[DirEntry]) -> Arc<Self> {
        let present = |name: &str| entries.iter().any(|e| e.file_name() == name);
        let names: Vec<&str> = IGNORE_FILES.into_iter().filter(|name| present(name)).collect();
        if names.is_empty() {
            return self.clone();
        }
        let mut builder = GitignoreBuilder::new(dir);
        for name in names {
            builder.add(dir.join(name));
        }
        let Ok(rules) = builder.build() else { return self.clone() };
        let mut chain = self.0.clone();
        chain.push(Arc::new(rules));
        Arc::new(Self(chain))
    }
}
