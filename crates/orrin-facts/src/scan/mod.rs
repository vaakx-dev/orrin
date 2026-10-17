mod catalog;
mod known;
mod path;
mod registry;
mod vswhere;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use orrin_core::model::{Tool, ToolSource};
use tokio::task::JoinSet;

pub use path::which;

use crate::book::{Book, Entry, modified, normalise, rank};
use crate::probe::{Probe, parse_version, probe};
use catalog::{SPECS, spec};

pub struct Found {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
    pub source: ToolSource,
}

pub async fn scan(book: &Book) -> Vec<Entry> {
    let found = tokio::task::spawn_blocking(gather).await.unwrap_or_default();
    let mut best: BTreeMap<String, Found> = BTreeMap::new();
    for item in found {
        let name = normalise(&item.name);
        if best.get(&name).is_none_or(|b| rank(item.source) < rank(b.source)) {
            best.insert(name, item);
        }
    }
    let mut probes = JoinSet::new();
    for (name, item) in best {
        let stamp = modified(&item.path);
        let known = item.version.clone().or_else(|| book.version_of(&item.path.to_string_lossy(), stamp));
        probes.spawn(async move { settle(name, item, known, stamp).await });
    }
    let mut entries: Vec<Entry> = probes.join_all().await.into_iter().flatten().collect();
    entries.sort_by(|a, b| a.tool.name.cmp(&b.tool.name));
    entries
}

async fn settle(name: String, item: Found, known: Option<String>, stamp: Option<u64>) -> Option<Entry> {
    let catalogued = spec(&name);
    let fallback = || catalogued.and_then(|_| hint(&item.path));
    let version = match (known, catalogued.and_then(|s| s.probe)) {
        (Some(version), _) => Some(version),
        (None, Some(args)) => match probe(&item.path, args).await {
            Probe::Version(version) => Some(version),
            Probe::Broken if item.path.to_string_lossy().contains("WindowsApps") => return None,
            _ => fallback(),
        },
        (None, None) => fallback(),
    };
    let tool = Tool { name, path: item.path.to_string_lossy().into_owned(), version, source: item.source };
    Some(Entry { tool, modified: stamp })
}

fn hint(path: &Path) -> Option<String> {
    let parent = path.parent()?.file_name()?.to_string_lossy().into_owned();
    let file = path.file_name()?.to_string_lossy().into_owned();
    parse_version(&file).or_else(|| parse_version(&parent))
}

fn gather() -> Vec<Found> {
    let mut found = Vec::new();
    for spec in SPECS {
        if let Some(path) = spec.exes().into_iter().find_map(which) {
            found.push(Found { name: spec.name.into(), path, version: None, source: ToolSource::Path });
        }
    }
    found.extend(registry::scan());
    found.extend(known::scan());
    found.extend(vswhere::scan());
    found
}

fn prefixed(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let runnable =
                if cfg!(windows) { name.ends_with(".exe") } else { !name.contains('.') || name.ends_with(".x86_64") };
            p.is_file() && name.starts_with(prefix) && runnable && !name.contains("console")
        })
        .collect();
    newest_first(&mut found);
    found
}

fn newest_first(paths: &mut [PathBuf]) {
    let key = |p: &PathBuf| -> Vec<u64> {
        parse_version(&p.to_string_lossy())
            .map(|v| v.split('.').filter_map(|n| n.parse().ok()).collect())
            .unwrap_or_default()
    };
    let length = |p: &PathBuf| p.as_os_str().len();
    paths.sort_by(|a, b| key(b).cmp(&key(a)).then_with(|| length(a).cmp(&length(b))).then_with(|| a.cmp(b)));
}
