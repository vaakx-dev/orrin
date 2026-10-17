use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use orrin_core::api::{Event, Log, ProjectsCreate, ProjectsSettings};
use orrin_core::model::{Change, FileState, ProjectSettings};
use orrin_core::service::{GcReport, Hub, Materialise, Projects, Service, Services, Setup, Store, Unavailable};
use orrin_core::{ChangeId, Config, ErrorCode, Events, Hash, Home, Identity, Result, SnapshotId};

#[derive(Default)]
struct MemStore(Mutex<HashMap<Hash, Vec<u8>>>);

impl Service for MemStore {}

impl Store for MemStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.0.lock().unwrap().insert(hash.clone(), bytes.to_vec());
        Ok(hash)
    }
    fn get(&self, hash: &Hash) -> Result<Vec<u8>> {
        Ok(self.0.lock().unwrap()[hash].clone())
    }
    fn put_file(&self, path: &Path) -> Result<Hash> {
        Unavailable("store").put_file(path)
    }
    fn size(&self, hash: &Hash) -> Result<u64> {
        Unavailable("store").size(hash)
    }
    fn has(&self, hash: &Hash) -> bool {
        self.0.lock().unwrap().contains_key(hash)
    }
    fn missing(&self, hashes: &[Hash]) -> Vec<Hash> {
        Unavailable("store").missing(hashes)
    }
    fn materialise(&self, h: &Hash, p: &Path, m: Materialise) -> Result<()> {
        Unavailable("store").materialise(h, p, m)
    }
    fn gc(&self, roots: &[Hash]) -> Result<GcReport> {
        Unavailable("store").gc(roots)
    }
    fn parts(&self, hash: &Hash) -> Result<Vec<Hash>> {
        Unavailable("store").parts(hash)
    }
    fn raw(&self, hash: &Hash) -> Result<Vec<u8>> {
        Unavailable("store").raw(hash)
    }
    fn put_raw(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        Unavailable("store").put_raw(hash, bytes)
    }
}

fn setup(dir: &Path) -> Setup {
    let home = Home::at(dir).unwrap();
    let me = Identity::load_or_create(&home).unwrap();
    Setup { home, config: Config::default(), me, events: Events::new(), hub: Hub::new() }
}

fn wire(setup: &Setup) -> Arc<dyn Projects> {
    let projects = crate::open(setup).unwrap();
    let none = Arc::new(Unavailable("test"));
    setup.hub.set(Services {
        home: setup.home.clone(),
        config: setup.config.clone(),
        me: setup.me.clone(),
        events: setup.events.clone(),
        store: Arc::new(MemStore::default()),
        projects: projects.clone(),
        views: none.clone(),
        landing: none.clone(),
        mergers: Vec::new(),
        index: none.clone(),
        facts: none.clone(),
        builds: none.clone(),
        cache: none.clone(),
        packages: none.clone(),
        peers: none.clone(),
        git: none,
    });
    projects
}

fn create(name: &str) -> ProjectsCreate {
    ProjectsCreate { name: name.into(), path: None, home_pc: None }
}

fn change(project: &orrin_core::ProjectId, parent: &SnapshotId, path: &str) -> Change {
    let state = FileState { hash: Hash::of(path.as_bytes()), size: 1, mode: Default::default() };
    Change {
        id: ChangeId::new(),
        project: project.clone(),
        parent: parent.clone(),
        snapshot: Hash::of(format!("{parent}{path}").as_bytes()),
        files: BTreeMap::from([(path.to_string(), Some(state))]),
        author: "me".into(),
        intent: "edit".into(),
        at: 1,
        pinned: false,
        undoes: None,
    }
}

#[tokio::test]
async fn creates_lists_and_sets_settings() {
    let dir = tempfile::tempdir().unwrap();
    let setup = setup(dir.path());
    let projects = wire(&setup);
    let mut events = setup.events.subscribe();

    let opened = projects.create(create("game")).await.unwrap();
    assert_eq!(opened.project.home_pc, setup.me.pc);
    assert!(matches!(events.recv().await.unwrap().event, Event::ProjectCreated { .. }));
    assert_eq!(projects.list().await.unwrap().projects, vec![opened.project.clone()]);
    assert_eq!(projects.create(create(" ")).await.unwrap_err().code, ErrorCode::Invalid);

    let set = ProjectSettings { ignore: vec!["target".into()], git: None };
    let id = opened.project.id.clone();
    projects.settings(ProjectsSettings { project: id.clone(), set: Some(set.clone()) }).await.unwrap();
    assert_eq!(projects.settings(ProjectsSettings { project: id, set: None }).await.unwrap(), set);
}

#[tokio::test]
async fn adopt_failure_rolls_back_create() {
    let dir = tempfile::tempdir().unwrap();
    let setup = setup(dir.path());
    let projects = wire(&setup);
    let input = ProjectsCreate { path: Some(dir.path().join("folder")), ..create("game") };
    assert_eq!(projects.create(input).await.unwrap_err().code, ErrorCode::Unavailable);
    assert!(projects.list().await.unwrap().projects.is_empty());
}

#[tokio::test]
async fn swaps_main_only_from_the_expected_parent() {
    let dir = tempfile::tempdir().unwrap();
    let setup = setup(dir.path());
    let projects = wire(&setup);
    let project = projects.create(create("game")).await.unwrap().project;
    let mut events = setup.events.subscribe();

    let first = change(&project.id, &project.main, "src/a.rs");
    let moved = projects.swap_main(first.clone()).await.unwrap();
    assert_eq!(moved.main, first.snapshot);
    assert!(matches!(events.recv().await.unwrap().event, Event::MainMoved { .. }));

    let stale = change(&project.id, &project.main, "b.txt");
    assert_eq!(projects.swap_main(stale.clone()).await.unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(projects.get(&project.id).await.unwrap().main, first.snapshot);
    assert_eq!(projects.change(&stale.id).await.unwrap_err().code, ErrorCode::NotFound);

    let second = change(&project.id, &first.snapshot, "b.txt");
    projects.swap_main(second.clone()).await.unwrap();
    projects.append(vec![first.clone()]).await.unwrap();
    projects.pin(&first.id, true).await.unwrap();
    assert!(projects.change(&first.id).await.unwrap().pinned);

    let log =
        |path: Option<&str>| Log { project: project.id.clone(), path: path.map(Into::into), before: None, limit: None };
    let all = projects.log(log(None)).await.unwrap().changes;
    assert_eq!(all.iter().map(|c| &c.id).collect::<Vec<_>>(), [&second.id, &first.id]);
    let in_src = projects.log(log(Some("src"))).await.unwrap().changes;
    assert_eq!(in_src.len(), 1);
    assert_eq!(in_src[0].id, first.id);
    let older = Log { before: Some(second.id.clone()), ..log(None) };
    assert_eq!(projects.log(older).await.unwrap().changes.len(), 1);
}
