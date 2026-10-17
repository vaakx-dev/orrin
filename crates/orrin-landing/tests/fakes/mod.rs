mod merger;
mod projects;
mod store;
mod views;

use std::collections::BTreeMap;
use std::sync::Arc;

use orrin_core::api::{Event, EventEnvelope};
use orrin_core::model::{Change, Entry, FileState, Project, Snapshot, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Hub, Projects, Services, Setup, Store, Unavailable};
use orrin_core::{ChangeId, Config, Events, Home, Identity, PcId, ProjectId, SnapshotId, ViewId};
use tokio::sync::broadcast::Receiver;

use merger::LineMerger;
use projects::FakeProjects;
use store::FakeStore;
use views::FakeViews;

pub struct Rig {
    pub hub: Hub,
    pub store: Arc<FakeStore>,
    pub projects: Arc<FakeProjects>,
    pub views: Arc<FakeViews>,
    pub project: ProjectId,
    events: Receiver<EventEnvelope>,
    _dir: tempfile::TempDir,
}

impl Rig {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        let me = Identity { pc: PcId::from("pc_home"), name: "home".into(), key: "key".into() };
        let store = Arc::new(FakeStore::default());
        let main = snapshot(&*store, files);
        let project = Project {
            id: ProjectId::new(),
            name: "game".into(),
            home_pc: me.pc.clone(),
            main,
            settings: Default::default(),
            created_at: 0,
        };
        let projects = Arc::new(FakeProjects::new(project.clone()));
        let views = Arc::new(FakeViews::default());
        let events = Events::new();
        let receiver = events.subscribe();
        let setup = Setup { home, config: Config::default(), me, events, hub: Hub::new() };
        setup.hub.set(Services {
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            events: setup.events.clone(),
            store: store.clone(),
            projects: projects.clone(),
            views: views.clone(),
            landing: orrin_landing::open(&setup).unwrap(),
            mergers: vec![Arc::new(LineMerger)],
            index: Arc::new(Unavailable("index")),
            facts: Arc::new(Unavailable("facts")),
            builds: Arc::new(Unavailable("builds")),
            cache: Arc::new(Unavailable("cache")),
            packages: Arc::new(Unavailable("packages")),
            peers: Arc::new(Unavailable("peers")),
            git: Arc::new(Unavailable("git")),
        });
        Self { hub: setup.hub, store, projects, views, project: project.id, events: receiver, _dir: dir }
    }

    pub fn main(&self) -> SnapshotId {
        self.projects.project.lock().unwrap().main.clone()
    }

    pub fn read(&self, path: &str) -> Option<String> {
        let entry = self.store.snapshot(&self.main()).unwrap().get(path).cloned()?;
        Some(String::from_utf8(self.store.get(&entry.hash).unwrap()).unwrap())
    }

    pub fn view(&self, kind: ViewKind, edits: &[(&str, Option<&str>)]) -> ViewId {
        let view = View {
            id: ViewId::new(),
            project: self.project.clone(),
            owner: "worker".into(),
            kind,
            path: Default::default(),
            base: self.main(),
            claims: Vec::new(),
            state: ViewState::Open,
            created_at: 0,
            landed: None,
        };
        let id = view.id.clone();
        self.views.add(view, self.edits(edits));
        id
    }

    pub async fn move_main(&self, edits: &[(&str, Option<&str>)]) {
        let main = self.store.snapshot(&self.main()).unwrap();
        let mut entries: BTreeMap<String, Entry> = main.entries.into_iter().map(|e| (e.path.clone(), e)).collect();
        let files = self.edits(edits);
        for (path, state) in &files {
            match state {
                Some(s) => entries.insert(
                    path.clone(),
                    Entry { path: path.clone(), hash: s.hash.clone(), size: s.size, mode: s.mode },
                ),
                None => entries.remove(path),
            };
        }
        let snapshot = self.store.put_snapshot(&Snapshot::new(entries.into_values().collect())).unwrap();
        let change = Change {
            id: ChangeId::new(),
            project: self.project.clone(),
            parent: self.main(),
            snapshot,
            files,
            author: "other".into(),
            intent: "moved main".into(),
            at: 0,
            pinned: false,
            undoes: None,
        };
        self.projects.swap_main(change.clone()).await.unwrap();
        self.projects.append(vec![change]).await.unwrap();
    }

    pub fn events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.events.try_recv().ok()).map(|e| e.event).collect()
    }

    fn edits(&self, edits: &[(&str, Option<&str>)]) -> Edits {
        edits.iter().map(|(path, text)| (path.to_string(), text.map(|t| file(&*self.store, t)))).collect()
    }
}

fn file(store: &dyn Store, text: &str) -> FileState {
    FileState { hash: store.put(text.as_bytes()).unwrap(), size: text.len() as u64, mode: Default::default() }
}

fn snapshot(store: &dyn Store, files: &[(&str, &str)]) -> SnapshotId {
    let entries = files.iter().map(|(path, text)| {
        let state = file(store, text);
        Entry { path: path.to_string(), hash: state.hash, size: state.size, mode: state.mode }
    });
    store.put_snapshot(&Snapshot::new(entries.collect())).unwrap()
}
