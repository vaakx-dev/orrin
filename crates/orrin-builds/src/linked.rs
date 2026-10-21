use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use orrin_core::model::{Artifact, Entry, Linked, Snapshot, Target};
use orrin_core::{Hash, ProjectId, Result, SnapshotId};

use crate::paths;

type Latest = BTreeMap<String, SnapshotId>;

pub struct Links {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Links {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, lock: Mutex::new(()) }
    }

    pub fn all(&self, project: &ProjectId) -> Vec<Linked> {
        let latest = self.load(project);
        latest.into_iter().map(|(target, files)| Linked { target, files }).collect()
    }

    pub fn set(&self, project: &ProjectId, linked: &Linked) -> Result<bool> {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut latest = self.load(project);
        if latest.get(&linked.target) == Some(&linked.files) {
            return Ok(false);
        }
        latest.insert(linked.target.clone(), linked.files.clone());
        let path = self.path(project);
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec(&latest)?)?;
        std::fs::rename(temp, path)?;
        Ok(true)
    }

    pub fn remove(&self, project: &ProjectId) -> Result<()> {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        match std::fs::remove_file(self.path(project)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }

    pub fn roots(&self) -> Vec<Hash> {
        let entries = std::fs::read_dir(&self.dir).into_iter().flatten().flatten();
        let files = entries.filter_map(|entry| std::fs::read(entry.path()).ok());
        files.filter_map(|bytes| serde_json::from_slice::<Latest>(&bytes).ok()).flat_map(|l| l.into_values()).collect()
    }

    fn load(&self, project: &ProjectId) -> Latest {
        let bytes = std::fs::read(self.path(project)).unwrap_or_default();
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    fn path(&self, project: &ProjectId) -> PathBuf {
        self.dir.join(format!("{}.json", project.as_str()))
    }
}

pub fn files(target: &Target, artifacts: &[Artifact]) -> Result<Snapshot> {
    let (outputs, scratch) = (paths::globs(&target.outputs)?, paths::globs(&target.scratch)?);
    let kept = artifacts.iter().filter(|a| outputs.is_match(&a.path) && !scratch.is_match(&a.path));
    let entries =
        kept.map(|a| Entry { path: a.path.clone(), hash: a.hash.clone(), size: a.size, mode: Default::default() });
    Ok(Snapshot::new(entries.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &str) -> Artifact {
        Artifact { path: path.into(), hash: Hash::of(path.as_bytes()), size: 1, modified: None }
    }

    #[test]
    fn keeps_outputs_without_scratch_and_remembers_the_latest() {
        let target = Target {
            outputs: vec!["node_modules/**".into()],
            scratch: vec!["node_modules/.cache/**".into()],
            ..Default::default()
        };
        let made = [artifact("node_modules/a/index.js"), artifact("node_modules/.cache/x"), artifact("log.txt")];
        let files = files(&target, &made).unwrap();
        let paths: Vec<&str> = files.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["node_modules/a/index.js"]);

        let dir = tempfile::tempdir().unwrap();
        let links = Links::new(dir.path().to_path_buf());
        let project = ProjectId::from("p_test");
        let linked = Linked { target: "install".into(), files: files.id() };
        assert!(links.set(&project, &linked).unwrap());
        assert!(!links.set(&project, &linked).unwrap());
        assert_eq!(links.all(&project), std::slice::from_ref(&linked));
        assert_eq!(links.roots(), [linked.files]);
    }
}
