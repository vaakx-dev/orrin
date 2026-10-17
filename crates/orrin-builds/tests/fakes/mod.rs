#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::{Change, Entry, Job, Project, ProjectSettings, Snapshot, Tool};
use orrin_core::service::{
    Builds, Facts, GcReport, Hub, Materialise, Projects, Service, Services, Setup, Store, Unavailable,
};
use orrin_core::{ChangeId, Config, Events, Hash, Home, Identity, JobId, ProjectId, Result};

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
        self.0.lock().unwrap().get(hash).cloned().ok_or_else(|| orrin_core::Error::not_found(hash.to_string()))
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
    fn materialise(&self, hash: &Hash, path: &Path, _: Materialise) -> Result<()> {
        Ok(std::fs::write(path, self.get(hash)?)?)
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
    fn put_raw(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(())
    }
}

pub struct FakeProjects(Mutex<Project>);

impl Service for FakeProjects {}

#[rustfmt::skip]
#[async_trait]
impl Projects for FakeProjects {
    async fn list(&self) -> Result<ProjectsListOutput> { unimplemented!() }
    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> { unimplemented!() }
    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> { unimplemented!() }
    async fn log(&self, _: Log) -> Result<LogOutput> { unimplemented!() }
    async fn get(&self, _: &ProjectId) -> Result<Project> { Ok(self.0.lock().unwrap().clone()) }
    async fn save(&self, _: Project) -> Result<()> { unimplemented!() }
    async fn swap_main(&self, _: Change) -> Result<Project> { unimplemented!() }
    async fn append(&self, _: Vec<Change>) -> Result<()> { unimplemented!() }
    async fn change(&self, _: &ChangeId) -> Result<Change> { unimplemented!() }
    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> { unimplemented!() }
    async fn thin(&self, _: &ProjectId) -> Result<u32> { unimplemented!() }
}

#[derive(Default)]
pub struct FakeFacts(pub Mutex<Vec<String>>);

impl Service for FakeFacts {}

#[rustfmt::skip]
#[async_trait]
impl Facts for FakeFacts {
    async fn list(&self, _: orrin_core::api::Facts) -> Result<FactsOutput> { unimplemented!() }
    async fn set(&self, _: FactsSet) -> Result<Tool> { unimplemented!() }
    async fn tool(&self, _: &str) -> Result<Option<Tool>> { Ok(None) }
    async fn learn(&self, name: &str, _: &str, _: Option<String>) -> Result<()> {
        self.0.lock().unwrap().push(name.to_string());
        Ok(())
    }
}

pub struct World {
    pub _dir: tempfile::TempDir,
    pub home: Home,
    pub builds: Arc<dyn Builds>,
    pub store: Arc<FakeStore>,
    pub projects: Arc<FakeProjects>,
    pub project: ProjectId,
    pub setup: Setup,
}

fn snapshot(store: &FakeStore, files: &[(&str, &str)]) -> Hash {
    let entries = files
        .iter()
        .map(|(path, body)| Entry {
            path: path.to_string(),
            hash: store.put(body.as_bytes()).unwrap(),
            size: body.len() as u64,
            mode: Default::default(),
        })
        .collect();
    store.put_snapshot(&Snapshot::new(entries)).unwrap()
}

pub fn world(files: &[(&str, &str)]) -> World {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path()).unwrap();
    let me = Identity::load_or_create(&home).unwrap();
    let setup =
        Setup { home: home.clone(), config: Config::default(), me: me.clone(), events: Events::new(), hub: Hub::new() };
    let builds = orrin_builds::open(&setup).unwrap();
    let store = Arc::new(FakeStore::default());
    let main = snapshot(&store, files);
    let project = Project {
        id: ProjectId::new(),
        name: "demo".into(),
        home_pc: me.pc.clone(),
        main,
        settings: Default::default(),
        created_at: 0,
    };
    let id = project.id.clone();
    let projects = Arc::new(FakeProjects(Mutex::new(project)));
    setup.hub.set(Services {
        home: home.clone(),
        config: setup.config.clone(),
        me,
        events: setup.events.clone(),
        store: store.clone(),
        projects: projects.clone(),
        views: Arc::new(Unavailable("views")),
        landing: Arc::new(Unavailable("landing")),
        mergers: Vec::new(),
        index: Arc::new(Unavailable("index")),
        facts: Arc::new(FakeFacts::default()),
        builds: builds.clone(),
        cache: Arc::new(Unavailable("cache")),
        packages: Arc::new(Unavailable("packages")),
        peers: Arc::new(Unavailable("peers")),
        git: Arc::new(Unavailable("git")),
    });
    builds.start().unwrap();
    World { _dir: dir, home, builds, store, projects, project: id, setup }
}

impl World {
    pub fn change(&self, files: &[(&str, &str)]) {
        self.projects.0.lock().unwrap().main = snapshot(&self.store, files);
    }

    pub fn restart(&self) -> Arc<dyn Builds> {
        let builds = orrin_builds::open(&self.setup).unwrap();
        builds.start().unwrap();
        builds
    }

    pub fn input(&self, target: &str) -> JobInput {
        JobInput {
            project: self.project.clone(),
            view: None,
            snapshot: None,
            target: target.into(),
            args: Vec::new(),
            pc: None,
        }
    }

    pub async fn finished(&self, job: &JobId) -> Job {
        for _ in 0..200 {
            let job = self.builds.status(JobRef { job: job.clone() }).await.unwrap();
            if job.state.is_done() {
                return job;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("job {job} did not finish");
    }
}
