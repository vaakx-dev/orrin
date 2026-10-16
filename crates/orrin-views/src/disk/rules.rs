use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{DirEntry, WalkBuilder};
use orrin_core::model::{MANIFEST_PATH, Manifest};

use crate::disk::files::is_temp;

pub struct Found {
    pub rel: String,
    pub abs: PathBuf,
    pub meta: std::fs::Metadata,
}

#[derive(Clone)]
struct Matchers {
    git: Arc<Gitignore>,
    include: Arc<Gitignore>,
    extra: Arc<Gitignore>,
}

pub struct Rules {
    root: PathBuf,
    matchers: Matchers,
    include_bases: Vec<String>,
}

impl Rules {
    pub fn load(root: &Path, settings: &[String]) -> Self {
        let manifest = read_manifest(root);
        let mut git = GitignoreBuilder::new(root);
        if root.join(".gitignore").is_file() {
            git.add(root.join(".gitignore"));
        }
        let extra = [".git/".to_string()].into_iter().chain(manifest.ignore).chain(settings.iter().cloned());
        let matchers = Matchers {
            git: Arc::new(git.build().unwrap_or_else(|_| Gitignore::empty())),
            include: Arc::new(matcher(root, manifest.include.iter().cloned())),
            extra: Arc::new(matcher(root, extra)),
        };
        let mut include_bases: Vec<String> = manifest.include.iter().map(|p| literal_base(p)).collect();
        include_bases.sort();
        include_bases.dedup();
        Self { root: root.to_path_buf(), matchers, include_bases }
    }

    pub fn ignores(&self, rel: &str, is_dir: bool) -> bool {
        is_temp(rel) || self.matchers.ignores(Path::new(rel), is_dir)
    }

    pub fn walk(&self) -> Vec<Found> {
        let mut found = BTreeMap::new();
        let main = self.walker(&self.root, true, |m, rel, is_dir| !m.ignores(rel, is_dir));
        let included = self.include_bases.iter().flat_map(|base| {
            self.walker(&self.root.join(base), false, |m, rel, is_dir| {
                !m.extra.matched_path_or_any_parents(rel, is_dir).is_ignore()
                    && (is_dir || m.include.matched_path_or_any_parents(rel, false).is_ignore())
            })
        });
        for entry in main.chain(included) {
            if let Some(file) = self.found(entry.path()) {
                found.entry(file.rel.clone()).or_insert(file);
            }
        }
        found.into_values().collect()
    }

    fn walker<K>(&self, start: &Path, standard: bool, keep: K) -> impl Iterator<Item = DirEntry> + use<K>
    where
        K: Fn(&Matchers, &Path, bool) -> bool + Send + Sync + 'static,
    {
        let matchers = self.matchers.clone();
        let root = self.root.clone();
        WalkBuilder::new(start)
            .standard_filters(standard)
            .hidden(false)
            .parents(false)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .filter_entry(move |entry| {
                let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
                let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                rel.as_os_str().is_empty() || (!is_temp_path(rel) && keep(&matchers, rel, is_dir))
            })
            .build()
            .flatten()
    }

    fn found(&self, abs: &Path) -> Option<Found> {
        let meta = std::fs::symlink_metadata(abs).ok().filter(|m| m.is_file())?;
        let rel = rel_path(&self.root, abs)?;
        Some(Found { rel, abs: abs.to_path_buf(), meta })
    }
}

impl Matchers {
    fn ignores(&self, rel: &Path, is_dir: bool) -> bool {
        self.extra.matched_path_or_any_parents(rel, is_dir).is_ignore()
            || (self.git.matched_path_or_any_parents(rel, is_dir).is_ignore()
                && !self.include.matched_path_or_any_parents(rel, is_dir).is_ignore())
    }
}

pub fn rel_path(root: &Path, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.is_empty() { None } else { Some(parts.join("/")) }
}

fn is_temp_path(rel: &Path) -> bool {
    rel.file_name().is_some_and(|name| is_temp(&name.to_string_lossy()))
}

fn matcher(root: &Path, lines: impl Iterator<Item = String>) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    for line in lines {
        let _ = builder.add_line(None, &line);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn literal_base(pattern: &str) -> String {
    let pattern = pattern.trim_start_matches('/');
    let literal = &pattern[..pattern.find(['*', '?', '[', '{', '\\']).unwrap_or(pattern.len())];
    literal.rfind('/').map(|end| literal[..end].to_string()).unwrap_or_default()
}

fn read_manifest(root: &Path) -> Manifest {
    std::fs::read_to_string(root.join(MANIFEST_PATH))
        .ok()
        .and_then(|text| toml::from_str::<Manifest>(&text).ok())
        .unwrap_or_default()
}
