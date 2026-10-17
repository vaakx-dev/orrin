mod fakes;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use fakes::{FakeLanding, FakeProjects, FakeStore};
use orrin_core::api::{GitExport, GitImport, SyncLand};
use orrin_core::model::{FileState, GitSettings, Mode, Project, ProjectSettings, Snapshot};
use orrin_core::service::{Hub, Services, Setup, Store, Unavailable};
use orrin_core::{Config, Events, Home, Identity, PcId, ProjectId};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=Tess", "-c", "user.email=tess@example.com"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

struct World {
    hub: Hub,
    store: Arc<FakeStore>,
    projects: Arc<FakeProjects>,
    project: ProjectId,
}

fn world(home: &Path, remote: &Path) -> World {
    let home = Home::at(home).unwrap();
    let store = Arc::new(FakeStore::default());
    let empty = store.put_snapshot(&Snapshot::default()).unwrap();
    let settings = ProjectSettings {
        git: Some(GitSettings { remote: remote.to_string_lossy().replace('\\', "/"), branch: "main".into() }),
        ..Default::default()
    };
    let project = Project {
        id: ProjectId::new(),
        name: "demo".into(),
        home_pc: PcId::new(),
        main: empty,
        settings,
        created_at: 0,
    };
    let id = project.id.clone();
    let projects = Arc::new(FakeProjects::new(project));
    let landing = Arc::new(FakeLanding { store: store.clone(), projects: projects.clone(), clock: AtomicU64::new(0) });
    let hub = Hub::new();
    let setup = Setup {
        home: home.clone(),
        config: Config::default(),
        me: Identity::load_or_create(&home).unwrap(),
        events: Events::new(),
        hub: hub.clone(),
    };
    let services = Services {
        home,
        config: setup.config.clone(),
        me: setup.me.clone(),
        events: setup.events.clone(),
        store: store.clone(),
        projects: projects.clone(),
        views: Arc::new(Unavailable("views")),
        landing,
        mergers: Vec::new(),
        index: Arc::new(Unavailable("index")),
        facts: Arc::new(Unavailable("facts")),
        builds: Arc::new(Unavailable("builds")),
        cache: Arc::new(Unavailable("cache")),
        peers: Arc::new(Unavailable("peers")),
        git: orrin_git::open(&setup).unwrap(),
    };
    hub.set(services);
    World { hub, store, projects, project: id }
}

impl World {
    async fn land(&self, intent: &str, files: &[(&str, Option<&[u8]>, Mode)]) {
        let mut changes = BTreeMap::new();
        for (path, bytes, mode) in files {
            let state =
                bytes.map(|b| FileState { hash: self.store.put(b).unwrap(), size: b.len() as u64, mode: *mode });
            changes.insert(path.to_string(), state);
        }
        let main = self.projects.state.lock().unwrap().0.main.clone();
        let input = SyncLand {
            project: self.project.clone(),
            parent: main,
            files: changes,
            author: "ana".into(),
            intent: intent.into(),
            claims: Vec::new(),
        };
        assert!(self.hub.landing.land_change(input).await.unwrap().landed);
    }

    fn main_file(&self, path: &str) -> Option<Vec<u8>> {
        let main = self.projects.state.lock().unwrap().0.main.clone();
        let entry = self.store.snapshot(&main).unwrap().get(path).cloned()?;
        Some(self.store.get(&entry.hash).unwrap())
    }
}

#[tokio::test]
async fn exports_changes_and_imports_a_commit() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--quiet", "--bare", "-b", "main", "remote.git"]);
    let w = world(&temp.path().join("home"), &remote);

    w.land("first", &[("a.txt", Some(b"one"), Mode::File), ("dir/run.sh", Some(b"echo"), Mode::Executable)]).await;
    w.land("second", &[("a.txt", Some(b"two"), Mode::File), ("dir/run.sh", None, Mode::File)]).await;
    let out = w.hub.git.export(GitExport { project: w.project.clone(), remote: None, branch: None }).await.unwrap();
    assert_eq!(out.commits, 2);

    assert_eq!(git(&remote, &["log", "--format=%s", "main"]), "second\nfirst\n");
    assert_eq!(git(&remote, &["show", "main:a.txt"]), "two");
    assert!(git(&remote, &["ls-tree", "-r", "main~1"]).contains("100755 blob"));
    assert!(!git(&remote, &["ls-tree", "-r", "main"]).contains("run.sh"));

    git(temp.path(), &["clone", "--quiet", "remote.git", "work"]);
    let work = temp.path().join("work");
    std::fs::write(work.join("c.txt"), "three").unwrap();
    git(&work, &["add", "c.txt"]);
    git(&work, &["commit", "--quiet", "-m", "third"]);
    git(&work, &["push", "--quiet", "origin", "main"]);

    let imported =
        w.hub.git.import(GitImport { project: w.project.clone(), remote: None, branch: None }).await.unwrap();
    assert_eq!(imported.changes.len(), 1);
    assert_eq!(w.main_file("c.txt").unwrap(), b"three");
    assert_eq!(w.main_file("a.txt").unwrap(), b"two");
    let last = w.projects.state.lock().unwrap().1.last().cloned().unwrap();
    assert_eq!(last.intent, "third");
    assert_eq!(last.author, "Tess <tess@example.com>");

    let again = w.hub.git.export(GitExport { project: w.project.clone(), remote: None, branch: None }).await.unwrap();
    assert_eq!(again.commits, 0);
    let none = w.hub.git.import(GitImport { project: w.project.clone(), remote: None, branch: None }).await.unwrap();
    assert!(none.changes.is_empty());
}
