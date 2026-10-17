use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orrin_core::api::*;
use orrin_core::model::*;
use orrin_core::service::*;
use orrin_core::{ChangeId, Error, Hash, PcId, ProjectId, Result, SnapshotId, ViewId, now_ms};

fn nope<T>() -> Result<T> {
    Err(Error::unavailable("fake"))
}

#[derive(Default)]
pub struct MemStore(Mutex<HashMap<Hash, Vec<u8>>>);

impl Service for MemStore {}

impl Store for MemStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(hash)
    }
    fn put_file(&self, _: &Path) -> Result<Hash> {
        nope()
    }
    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.0.lock().unwrap().get(hash).cloned().ok_or_else(|| Error::not_found(hash.to_string()))
    }
    fn size(&self, hash: &Hash) -> Result<u64> {
        self.get(hash).map(|b| b.len() as u64)
    }
    fn has(&self, hash: &Hash) -> bool {
        self.0.lock().unwrap().contains_key(hash)
    }
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        hashes.iter().filter(|h| !self.has(h)).cloned().collect()
    }
    fn materialise(&self, _: &Hash, _: &Path, _: Materialise) -> Result<()> {
        nope()
    }
    fn gc(&self, _: &[Hash]) -> Result<GcReport> {
        nope()
    }
    fn parts(&self, _: &Hash) -> Result<Vec<Hash>> {
        nope()
    }
    fn raw(&self, hash: &Hash) -> Result<Vec<u8>> {
        self.get(hash)
    }
    fn put_raw(&self, _: &Hash, _: &[u8]) -> Result<()> {
        nope()
    }
}

impl MemStore {
    pub fn snapshot_of(&self, files: &[(&str, impl AsRef<[u8]>)]) -> SnapshotId {
        let entries = files
            .iter()
            .map(|(path, body)| Entry {
                path: path.to_string(),
                hash: self.put(body.as_ref()).unwrap(),
                size: body.as_ref().len() as u64,
                mode: Mode::File,
            })
            .collect();
        self.put_snapshot(&Snapshot::new(entries)).unwrap()
    }
}

pub struct OneProject(pub Mutex<Project>);

impl OneProject {
    pub fn new(main: SnapshotId) -> Self {
        Self(Mutex::new(Project {
            id: ProjectId::from("p_test"),
            name: "test".into(),
            home_pc: PcId::from("pc_test"),
            main,
            settings: ProjectSettings::default(),
            created_at: now_ms(),
        }))
    }
}

impl Service for OneProject {}

#[async_trait]
impl Projects for OneProject {
    async fn list(&self) -> Result<ProjectsListOutput> {
        Ok(ProjectsListOutput { projects: vec![self.0.lock().unwrap().clone()] })
    }
    async fn create(&self, _: ProjectsCreate) -> Result<ProjectOpened> {
        nope()
    }
    async fn settings(&self, _: ProjectsSettings) -> Result<ProjectSettings> {
        nope()
    }
    async fn log(&self, _: Log) -> Result<LogOutput> {
        nope()
    }
    async fn get(&self, _: &ProjectId) -> Result<Project> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn save(&self, _: Project) -> Result<()> {
        nope()
    }
    async fn swap_main(&self, _: Change) -> Result<Project> {
        nope()
    }
    async fn append(&self, _: Vec<Change>) -> Result<()> {
        nope()
    }
    async fn change(&self, _: &ChangeId) -> Result<Change> {
        nope()
    }
    async fn pin(&self, _: &ChangeId, _: bool) -> Result<()> {
        nope()
    }
    async fn thin(&self, _: &ProjectId) -> Result<u32> {
        nope()
    }
}

#[derive(Default)]
pub struct FolderViews(pub Mutex<BTreeMap<String, Option<String>>>, pub AtomicBool);

impl FolderViews {
    pub fn lock_records(&self, locked: bool) {
        self.1.store(locked, Ordering::SeqCst);
    }

    pub fn write(&self, path: &str, body: Option<&str>) {
        self.0.lock().unwrap().insert(path.into(), body.map(str::to_string));
    }
}

impl Service for FolderViews {}

#[async_trait]
impl Views for FolderViews {
    async fn import(&self, _: ProjectsImport) -> Result<ProjectOpened> {
        nope()
    }
    async fn create(&self, _: ViewCreate) -> Result<View> {
        nope()
    }
    async fn list(&self, _: ViewList) -> Result<ViewListOutput> {
        nope()
    }
    async fn remove(&self, _: ViewDrop) -> Result<Done> {
        nope()
    }
    async fn read(&self, input: Read) -> Result<ReadOutput> {
        let body = self.0.lock().unwrap().get(&input.path).cloned().flatten();
        let body = body.ok_or_else(|| Error::not_found(input.path.clone()))?;
        Ok(ReadOutput {
            clock: Default::default(),
            path: input.path,
            hash: Hash::of(body.as_bytes()),
            size: body.len() as u64,
            binary: false,
            total_lines: None,
            encoding: Encoding::Utf8,
            content: Some(body),
        })
    }
    async fn patch(&self, _: Patch) -> Result<Written> {
        nope()
    }
    async fn create_file(&self, _: Create) -> Result<Written> {
        nope()
    }
    async fn move_file(&self, _: Move) -> Result<Touched> {
        nope()
    }
    async fn delete(&self, _: Delete) -> Result<Touched> {
        nope()
    }
    async fn changes(&self, _: Changes) -> Result<ChangesOutput> {
        nope()
    }
    async fn diff(&self, _: Diff) -> Result<DiffOutput> {
        nope()
    }
    async fn get(&self, id: &ViewId) -> Result<View> {
        Ok(View {
            id: id.clone(),
            project: ProjectId::from("p_test"),
            owner: "test".into(),
            kind: ViewKind::Worker,
            path: PathBuf::new(),
            base: Hash::of(b""),
            claims: vec![],
            state: ViewState::Open,
            created_at: 0,
            landed: None,
        })
    }
    async fn adopt(&self, _: &ProjectId, _: &Path) -> Result<View> {
        nope()
    }
    async fn capture(&self, _: &ViewId) -> Result<Edits> {
        assert!(!self.1.load(Ordering::SeqCst), "capture while the view record is locked would deadlock");
        let files = self.0.lock().unwrap().clone();
        Ok(files
            .into_iter()
            .map(|(path, body)| {
                let state =
                    body.map(|b| FileState { hash: Hash::of(b.as_bytes()), size: b.len() as u64, mode: Mode::File });
                (path, state)
            })
            .collect())
    }
    async fn advance(&self, _: &ViewId, _: &SnapshotId) -> Result<Vec<String>> {
        nope()
    }
    async fn landed(&self, _: &ViewId, _: &ChangeId) -> Result<()> {
        nope()
    }
    async fn link(&self, _: &ProjectId, _: Linked) -> Result<()> {
        nope()
    }
}

pub struct OneTool;

impl Service for OneTool {}

#[async_trait]
impl orrin_core::service::Facts for OneTool {
    async fn list(&self, _: orrin_core::api::Facts) -> Result<FactsOutput> {
        nope()
    }
    async fn set(&self, _: FactsSet) -> Result<Tool> {
        nope()
    }
    async fn tool(&self, name: &str) -> Result<Option<Tool>> {
        Ok((name == "blender").then(|| Tool {
            name: "blender".into(),
            path: "C:/Blender/blender.exe".into(),
            version: Some("4.2".into()),
            source: ToolSource::KnownDir,
        }))
    }
    async fn learn(&self, _: &str, _: &str, _: Option<String>) -> Result<()> {
        Ok(())
    }
}

pub struct World {
    pub hub: Hub,
    pub setup: Setup,
    pub store: Arc<MemStore>,
    pub project: Arc<OneProject>,
    pub views: Arc<FolderViews>,
}

pub fn world(home: &Path, files: &[(&str, impl AsRef<[u8]>)]) -> World {
    let home = orrin_core::Home::at(home).unwrap();
    let hub = Hub::new();
    let setup = Setup {
        home,
        config: orrin_core::Config::default(),
        me: orrin_core::Identity { pc: PcId::from("pc_test"), name: "test".into(), key: "k".into() },
        events: orrin_core::Events::new(),
        hub: hub.clone(),
    };
    let store = Arc::new(MemStore::default());
    let project = Arc::new(OneProject::new(store.snapshot_of(files)));
    World { hub, setup, store, project, views: Arc::new(FolderViews::default()) }
}

impl World {
    pub fn wire(&self, index: Arc<dyn Index>) {
        let none = Unavailable("fake");
        self.hub.set(Services {
            home: self.setup.home.clone(),
            config: self.setup.config.clone(),
            me: self.setup.me.clone(),
            events: self.setup.events.clone(),
            store: self.store.clone(),
            projects: self.project.clone(),
            views: self.views.clone(),
            landing: Arc::new(none),
            mergers: vec![],
            index,
            facts: Arc::new(OneTool),
            builds: Arc::new(none),
            peers: Arc::new(none),
            git: Arc::new(none),
        });
    }
}
