use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use orrin_core::model::{MANIFEST_PATH, Manifest};

use crate::disk::files::is_temp;

pub struct Found {
    pub rel: String,
    pub abs: PathBuf,
    pub meta: std::fs::Metadata,
}

pub struct Rules {
    root: PathBuf,
    extra: Arc<Gitignore>,
}

impl Rules {
    pub fn load(root: &Path, settings: &[String]) -> Self {
        let mut builder = GitignoreBuilder::new(root);
        let _ = builder.add_line(None, ".git/");
        if root.join(".gitignore").is_file() {
            builder.add(root.join(".gitignore"));
        }
        for line in manifest_ignores(root).iter().chain(settings) {
            let _ = builder.add_line(None, line);
        }
        let extra = builder.build().unwrap_or_else(|_| Gitignore::empty());
        Self { root: root.to_path_buf(), extra: Arc::new(extra) }
    }

    pub fn ignores(&self, rel: &str, is_dir: bool) -> bool {
        is_temp(rel) || self.extra.matched_path_or_any_parents(rel, is_dir).is_ignore()
    }

    pub fn walk(&self) -> Vec<Found> {
        let extra = self.extra.clone();
        let root = self.root.clone();
        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .parents(false)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .filter_entry(move |entry| {
                let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
                let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                rel.as_os_str().is_empty()
                    || !(is_temp_path(rel) || extra.matched_path_or_any_parents(rel, is_dir).is_ignore())
            })
            .build();
        walker.flatten().filter_map(|entry| self.found(entry.path())).collect()
    }

    fn found(&self, abs: &Path) -> Option<Found> {
        let meta = std::fs::symlink_metadata(abs).ok().filter(|m| m.is_file())?;
        let rel = rel_path(&self.root, abs)?;
        Some(Found { rel, abs: abs.to_path_buf(), meta })
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

fn manifest_ignores(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join(MANIFEST_PATH))
        .ok()
        .and_then(|text| toml::from_str::<Manifest>(&text).ok())
        .map(|manifest| manifest.ignore)
        .unwrap_or_default()
}
