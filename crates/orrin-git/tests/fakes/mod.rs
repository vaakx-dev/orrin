use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::{Change, Entry, Project, ProjectSettings, Snapshot};
use orrin_core::service::{GcReport, Landing, Materialise, Projects, Service, Store};
use orrin_core::{ChangeId, Error, Hash, ProjectId, Result};

#[derive(Default)]
pub struct FakeStore(Mutex<HashMap<Hash, Vec<u8>>>);

impl Service for FakeStore {}

impl Store for FakeStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(hash)
    }
    fn put_file(&self, path: &Path) -> Result<Hash> {
        self.put(&std::fs::read(path)?)
    }
    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.0.lock().unwrap().get(hash).cloned().ok_or_else(|| Error::not_found(hash.to_string()))
    }
    fn size(&self, hash: &Hash) -> Result<u64> {
        Ok(self.get(hash)?.len() as u64)
    }
    fn has(&self, hash: &Hash) -> bool {
        self.0.lock().unwrap().contains_key(hash)
    }
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        hashes.iter().filter(|h| !self.has(h)).cloned().collect()
    }
    fn materialise(&self, _: &Hash, _: &Path, _: Materialise) -> Result<()> {
        unimplemented!()
    }
    fn gc(&self, _: &[Hash]) -> Result<GcReport> {
        unimplemented!()
    }
    fn parts(&self, _: &Hash) -> Result<Vec<Hash>> {
        Ok(Vec::new())
    }
    fn raw(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.get(hash)
    }
    fn put_raw(&self, _: &Hash, bytes: &[u8]) -> Result<()> {
        self.put(bytes).map(|_| ())
    }
}

pub struct FakeProjects {
    pub state: Mutex<(Project, Vec<Change>)>,
}

impl FakeProjects {
    pub fn new(project: Project) -> Self {
        Self { state: Mutex::new((project, Vec::new())) }
    }
}

impl Service for FakeProjects {}

#[async_trait]
impl Projects for FakeProjects {
    async fn list(&self) -> Result<ProjectsListOutput> {
        unimplemented!()
    }
    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> {
        unimplemented!()
    }
    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> {
        unimplemented!()
    }
    async fn log(&self, input: Log) -> Result<LogOutput> {
        let state = self.state.lock().unwrap();
        let newest: Vec<Change> = state.1.iter().rev().cloned().collect();
        let start = input.before.and_then(|b| newest.iter().position(|c| c.id == b).map(|i| i + 1)).unwrap_or(0);
        let limit = input.limit.unwrap_or(u32::MAX) as usize;
        Ok(LogOutput { changes: newest.into_iter().skip(start).take(limit).collect() })
    }
    async fn get(&self, _: &ProjectId) -> Result<Project> {
        Ok(self.state.lock().unwrap().0.clone())
    }
    async fn save(&self, project: Project) -> Result<()> {
        self.state.lock().unwrap().0 = project;
        Ok(())
    }
    async fn swap_main(&self, change: Change) -> Result<Project> {
        let mut state = self.state.lock().unwrap();
        if state.0.main != change.parent {
            return Err(Error::conflict("main moved"));
        }
        state.0.main = change.snapshot.clone();
        state.1.push(change);
        Ok(state.0.clone())
    }
    async fn append(&self, _: Vec<Change>) -> Result<()> {
        unimplemented!()
    }
    async fn change(&self, _: &ChangeId) -> Result<Change> {
        unimplemented!()
    }
    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> {
        unimplemented!()
    }
    async fn thin(&self, _: &ProjectId) -> Result<u32> {
        unimplemented!()
    }
}

pub struct FakeLanding {
    pub store: Arc<FakeStore>,
    pub projects: Arc<FakeProjects>,
    pub clock: AtomicU64,
}

impl Service for FakeLanding {}

#[async_trait]
impl Landing for FakeLanding {
    async fn land(&self, _: ViewLand) -> Result<LandOutput> {
        unimplemented!()
    }
    async fn undo(&self, _: Undo) -> Result<LandOutput> {
        unimplemented!()
    }
    async fn land_change(&self, input: SyncLand) -> Result<LandOutput> {
        let main = self.projects.get(&input.project).await?.main;
        let mut entries: HashMap<String, Entry> =
            self.store.snapshot(&main)?.entries.into_iter().map(|e| (e.path.clone(), e)).collect();
        for (path, state) in &input.files {
            match state {
                Some(s) => entries.insert(
                    path.clone(),
                    Entry { path: path.clone(), hash: s.hash.clone(), size: s.size, mode: s.mode },
                ),
                None => entries.remove(path),
            };
        }
        let snapshot = self.store.put_snapshot(&Snapshot::new(entries.into_values().collect()))?;
        let change = Change {
            id: ChangeId::new(),
            project: input.project,
            parent: main,
            snapshot: snapshot.clone(),
            files: input.files,
            author: input.author,
            intent: input.intent,
            at: 1_700_000_000_000 + self.clock.fetch_add(1000, Ordering::SeqCst),
            pinned: false,
            undoes: None,
        };
        self.projects.swap_main(change.clone()).await?;
        Ok(LandOutput {
            landed: true,
            change: Some(change),
            main: snapshot,
            conflicts: Vec::new(),
            skipped: Vec::new(),
        })
    }
}
