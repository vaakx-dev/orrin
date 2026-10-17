use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::{Change, Project, ProjectSettings};
use orrin_core::service::{
    GcReport, Hub, Index, Materialise, Projects, Service, Services, Setup, Store, Unavailable, Views,
};
use orrin_core::{ChangeId, Clock, Config, Error, Events, Hash, Home, Identity, PcId, ProjectId, Result, ViewId};
use tempfile::TempDir;

use crate::inner::Inner;
use crate::service::ViewsService;

pub struct Rig {
    pub dir: TempDir,
    pub setup: Setup,
    pub hub: Hub,
    pub inner: Arc<Inner>,
    pub views: Arc<dyn Views>,
    pub projects: Arc<FakeProjects>,
}

impl Rig {
    pub fn new(big_file: u64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path().join("home")).unwrap();
        let mut config = Config::default();
        config.views.big_file = big_file;
        config.views.keep_days = 0;
        let me = Identity { pc: PcId::from("pc_test"), name: "test".into(), key: "k".into() };
        let setup = Setup {
            home: home.clone(),
            config: config.clone(),
            me: me.clone(),
            events: Events::new(),
            hub: Hub::new(),
        };
        let inner = Arc::new(Inner::new(&setup).unwrap());
        let views: Arc<dyn Views> = Arc::new(ViewsService(inner.clone()));
        let projects = Arc::new(FakeProjects::default());
        setup.hub.set(Services {
            home,
            config,
            me,
            events: setup.events.clone(),
            store: Arc::new(FakeStore::new(dir.path().join("links"))),
            projects: projects.clone(),
            views: views.clone(),
            landing: Arc::new(Unavailable("landing")),
            mergers: Vec::new(),
            index: Arc::new(FakeIndex::default()),
            facts: Arc::new(Unavailable("facts")),
            builds: Arc::new(Unavailable("builds")),
            peers: Arc::new(Unavailable("peers")),
            git: Arc::new(Unavailable("git")),
        });
        Self { dir, hub: setup.hub.clone(), setup, inner, views, projects }
    }

    pub fn folder(&self, files: &[(&str, &str)]) -> PathBuf {
        let root = self.dir.path().join("project");
        for (path, text) in files {
            let abs = root.join(path);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, text).unwrap();
        }
        root
    }
}

pub struct FakeStore {
    blobs: Mutex<HashMap<Hash, Vec<u8>>>,
    links: PathBuf,
}

impl FakeStore {
    fn new(links: PathBuf) -> Self {
        std::fs::create_dir_all(&links).unwrap();
        Self { blobs: Mutex::default(), links }
    }
}

impl Service for FakeStore {}

impl Store for FakeStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.blobs.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(hash)
    }
    fn put_file(&self, path: &Path) -> Result<Hash> {
        self.put(&std::fs::read(path)?)
    }
    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.blobs.lock().unwrap().get(hash).cloned().ok_or_else(|| Error::not_found(hash.to_string()))
    }
    fn size(&self, hash: &Hash) -> Result<u64> {
        Ok(self.get(hash)?.len() as u64)
    }
    fn has(&self, hash: &Hash) -> bool {
        self.blobs.lock().unwrap().contains_key(hash)
    }
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        hashes.iter().filter(|h| !self.has(h)).cloned().collect()
    }
    fn materialise(&self, hash: &Hash, path: &Path, how: Materialise) -> Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        if how != Materialise::Link {
            return Ok(std::fs::write(path, self.get(hash)?)?);
        }
        let copy = self.links.join(hash.as_str());
        if !copy.exists() {
            std::fs::write(&copy, self.get(hash)?)?;
            let mut perms = std::fs::metadata(&copy)?.permissions();
            perms.set_readonly(true);
            std::fs::set_permissions(&copy, perms)?;
        }
        Ok(std::fs::hard_link(&copy, path)?)
    }
    fn gc(&self, _: &[Hash]) -> Result<GcReport> {
        Ok(GcReport::default())
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

#[derive(Default)]
pub struct FakeProjects {
    projects: Mutex<HashMap<ProjectId, Project>>,
    pub changes: Mutex<HashMap<ChangeId, Change>>,
}

impl Service for FakeProjects {}

#[async_trait]
impl Projects for FakeProjects {
    async fn list(&self) -> Result<ProjectsListOutput> {
        Ok(ProjectsListOutput { projects: self.projects.lock().unwrap().values().cloned().collect() })
    }
    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> {
        Err(Error::unavailable("fake"))
    }
    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> {
        Err(Error::unavailable("fake"))
    }
    async fn log(&self, _: Log) -> Result<LogOutput> {
        Err(Error::unavailable("fake"))
    }
    async fn get(&self, id: &ProjectId) -> Result<Project> {
        self.projects.lock().unwrap().get(id).cloned().ok_or_else(|| Error::not_found(id.to_string()))
    }
    async fn save(&self, project: Project) -> Result<()> {
        self.projects.lock().unwrap().insert(project.id.clone(), project);
        Ok(())
    }
    async fn swap_main(&self, _: Change) -> Result<Project> {
        Err(Error::unavailable("fake"))
    }
    async fn append(&self, changes: Vec<Change>) -> Result<()> {
        let mut all = self.changes.lock().unwrap();
        changes.into_iter().for_each(|c| {
            all.insert(c.id.clone(), c);
        });
        Ok(())
    }
    async fn change(&self, id: &ChangeId) -> Result<Change> {
        self.changes.lock().unwrap().get(id).cloned().ok_or_else(|| Error::not_found(id.to_string()))
    }
    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> {
        Ok(())
    }
    async fn thin(&self, _: &ProjectId) -> Result<u32> {
        Ok(0)
    }
}

#[derive(Default)]
pub struct FakeIndex(AtomicU64);

impl Service for FakeIndex {}

#[async_trait]
impl Index for FakeIndex {
    async fn find(&self, _: Find) -> Result<FindOutput> {
        Err(Error::unavailable("fake"))
    }
    async fn search(&self, _: Search) -> Result<SearchOutput> {
        Err(Error::unavailable("fake"))
    }
    async fn outline(&self, _: Outline) -> Result<OutlineOutput> {
        Err(Error::unavailable("fake"))
    }
    async fn locate(&self, _: Where) -> Result<WhereOutput> {
        Err(Error::unavailable("fake"))
    }
    async fn touched(&self, _: &ProjectId, _: Option<&ViewId>, _: &[String]) -> Result<Clock> {
        Ok(Clock(self.0.fetch_add(1, Ordering::SeqCst) + 1))
    }
    async fn clock(&self, _: &ProjectId, _: Option<&ViewId>) -> Clock {
        Clock(self.0.load(Ordering::SeqCst))
    }
}
