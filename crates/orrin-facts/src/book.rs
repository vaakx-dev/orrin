use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use orrin_core::model::{Tool, ToolSource};
use orrin_core::{Millis, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub tool: Tool,
    #[serde(default)]
    pub modified: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Book {
    #[serde(default)]
    pub refreshed: Millis,
    #[serde(default)]
    pub tools: BTreeMap<String, Entry>,
}

pub fn normalise(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower.strip_suffix(".exe").map(str::to_string).unwrap_or(lower)
}

pub fn modified(path: &Path) -> Option<u64> {
    let time = std::fs::metadata(path).ok()?.modified().ok()?;
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

pub fn rank(source: ToolSource) -> u8 {
    match source {
        ToolSource::Manual => 0,
        ToolSource::Learned => 1,
        ToolSource::Path => 2,
        ToolSource::Registry => 3,
        ToolSource::KnownDir => 4,
    }
}

impl Entry {
    pub fn new(tool: Tool) -> Self {
        let modified = modified(Path::new(&tool.path));
        Self { tool, modified }
    }
}

impl Book {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    pub fn tools(&self) -> Vec<Tool> {
        self.tools.values().map(|e| e.tool.clone()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.get(&normalise(name)).map(|e| &e.tool)
    }

    pub fn version_of(&self, path: &str, stamp: Option<u64>) -> Option<String> {
        self.tools
            .values()
            .find(|e| e.tool.path == path && e.modified == stamp && stamp.is_some())
            .and_then(|e| e.tool.version.clone())
    }

    pub fn put(&mut self, mut entry: Entry) -> bool {
        let name = normalise(&entry.tool.name);
        if let Some(existing) = self.tools.get(&name) {
            if rank(entry.tool.source) > rank(existing.tool.source) {
                return false;
            }
            if entry.tool.version.is_none() && existing.tool.path == entry.tool.path {
                entry.tool.version = existing.tool.version.clone();
            }
        }
        entry.tool.name = name.clone();
        self.tools.insert(name, entry);
        true
    }

    pub fn replace_scanned(&mut self, scanned: Vec<Entry>, at: Millis) {
        self.tools.retain(|_, e| {
            matches!(e.tool.source, ToolSource::Manual | ToolSource::Learned) && Path::new(&e.tool.path).exists()
        });
        for entry in scanned {
            self.put(entry);
        }
        self.refreshed = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, path: &str, source: ToolSource) -> Entry {
        Entry { tool: Tool { name: name.into(), path: path.into(), version: None, source }, modified: None }
    }

    #[test]
    fn manual_beats_learned_beats_scanned() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("blender.exe");
        std::fs::write(&exe, "").unwrap();
        let exe = exe.to_string_lossy().to_string();
        let mut book = Book::default();
        assert!(book.put(tool("Blender.exe", &exe, ToolSource::Manual)));
        assert!(!book.put(tool("blender", "other", ToolSource::Learned)));
        book.replace_scanned(
            vec![tool("blender", "scanned", ToolSource::Path), tool("git", "git", ToolSource::Path)],
            1,
        );
        assert_eq!(book.get("BLENDER").unwrap().path, exe);
        assert_eq!(book.get("git.exe").unwrap().source, ToolSource::Path);

        let file = dir.path().join("facts.json");
        book.save(&file).unwrap();
        assert_eq!(Book::load(&file).tools(), book.tools());
    }
}
