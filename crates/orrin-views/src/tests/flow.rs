use std::time::{Duration, UNIX_EPOCH};

use orrin_core::api::*;
use orrin_core::model::{
    Artifact, Change, Entry, FileState, FileStatus, Linked, SHARE_FROM, Snapshot, View, ViewState,
};
use orrin_core::service::{Edits, Projects};
use orrin_core::{ChangeId, ProjectId, now_ms};

use crate::inner::Inner;
use crate::snapshots::with_edits;
use crate::tests::fakes::Rig;

async fn import(rig: &Rig, files: &[(&str, &str)]) -> ProjectOpened {
    let path = rig.folder(files);
    rig.views.import(ProjectsImport { path, name: None }).await.unwrap()
}

async fn worker(rig: &Rig, project: &ProjectId) -> View {
    rig.views
        .create(ViewCreate {
            project: project.clone(),
            owner: "agent".into(),
            base: None,
            base_view: None,
            claims: vec![],
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn a_view_based_on_a_view_starts_from_its_edits_and_hands_edits_back() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "hello\n")]).await;
    let project = opened.project.id.clone();
    let parent = worker(&rig, &project).await;
    rig.views.patch(edit(&project, &parent, "a.txt", "hello", "parent")).await.unwrap();
    let from_parent = ViewCreate {
        project: project.clone(),
        owner: "helper".into(),
        base: None,
        base_view: Some(parent.id.clone()),
        claims: vec![],
    };
    let child = rig.views.create(from_parent).await.unwrap();
    assert_eq!(child.base_view.as_ref(), Some(&parent.id));
    assert_eq!(std::fs::read_to_string(child.path.join("a.txt")).unwrap(), "parent\n");

    std::fs::write(child.path.join("a.txt"), "child\n").unwrap();
    let files = rig.views.capture(&child.id).await.unwrap();
    assert!(rig.views.take(&parent.id, &child.base, &files).await.unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(parent.path.join("a.txt")).unwrap(), "child\n");
    assert_eq!(rig.views.take(&parent.id, &child.base, &files).await.unwrap(), ["a.txt"]);
}

fn edit(project: &ProjectId, view: &View, path: &str, old: &str, new: &str) -> Patch {
    let edits = vec![TextEdit { old: old.into(), new: new.into(), all: false }];
    Patch { project: project.clone(), view: view.id.clone(), path: path.into(), edits }
}

fn statuses(out: &ChangesOutput) -> Vec<(String, FileStatus)> {
    out.files.iter().map(|f| (f.path.clone(), f.status)).collect()
}

#[tokio::test]
async fn import_respects_ignores() {
    let rig = Rig::new(1 << 20);
    let files = [
        ("a.txt", "hello\n"),
        (".gitignore", "build/\n"),
        ("build/out.bin", "x"),
        (".git/config", "x"),
        ("orrin.toml", "ignore = [\"*.log\"]\n"),
        ("x.log", "x"),
        ("sub/b.txt", "b\n"),
        ("sub/.gitignore", "*.tmp\n"),
        ("sub/c.tmp", "x"),
    ];
    let opened = import(&rig, &files).await;
    let main = rig.hub.store.snapshot(&opened.project.main).unwrap();
    let paths: Vec<&str> = main.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, [".gitignore", "a.txt", "orrin.toml", "sub/.gitignore", "sub/b.txt"]);
    let user = opened.view.unwrap();
    let changes = rig.views.changes(Changes { project: opened.project.id, view: user.id }).await.unwrap();
    assert!(changes.files.is_empty());
}

#[tokio::test]
async fn import_brings_back_included_paths() {
    let rig = Rig::new(1 << 20);
    let files = [
        (".gitignore", "extern/\n*.obj\n"),
        ("extern/lib/src/a.cpp", "a\n"),
        ("extern/lib/src/a.obj", "x"),
        ("extern/lib/bin/a.lib", "x"),
        ("extern/other/b.cpp", "b\n"),
        ("orrin.toml", "include = [\"extern/lib/**\"]\nignore = [\"extern/lib/bin/\", \"*.obj\"]\n"),
    ];
    let opened = import(&rig, &files).await;
    let main = rig.hub.store.snapshot(&opened.project.main).unwrap();
    let paths: Vec<&str> = main.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, [".gitignore", "extern/lib/src/a.cpp", "orrin.toml"]);
    let user = opened.view.unwrap();
    let changes = rig.views.changes(Changes { project: opened.project.id, view: user.id }).await.unwrap();
    assert!(changes.files.is_empty());
}

#[tokio::test]
async fn worker_edits_show_in_changes_and_diff() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "hello\n"), ("sub/b.txt", "b\n")]).await;
    let project = opened.project.id.clone();
    let view = worker(&rig, &project).await;
    let written = rig.views.patch(edit(&project, &view, "a.txt", "hello", "bye")).await.unwrap();
    assert!(written.clock.0 > 0);
    let create = Create {
        project: project.clone(),
        view: view.id.clone(),
        path: "new.txt".into(),
        content: "new\n".into(),
        encoding: Default::default(),
        overwrite: false,
    };
    rig.views.create_file(create).await.unwrap();
    let moved =
        Move { project: project.clone(), view: view.id.clone(), from: "new.txt".into(), to: "docs/new.txt".into() };
    rig.views.move_file(moved).await.unwrap();
    rig.views
        .delete(Delete { project: project.clone(), view: view.id.clone(), path: "sub/b.txt".into() })
        .await
        .unwrap();

    let changes = rig.views.changes(Changes { project: project.clone(), view: view.id.clone() }).await.unwrap();
    let expected = [
        ("a.txt".to_string(), FileStatus::Modified),
        ("docs/new.txt".to_string(), FileStatus::Added),
        ("sub/b.txt".to_string(), FileStatus::Deleted),
    ];
    assert_eq!(statuses(&changes), expected);

    let diff = Diff { project: project.clone(), view: Some(view.id.clone()), change: None, path: Some("a.txt".into()) };
    let patch = rig.views.diff(diff).await.unwrap().files[0].patch.clone().unwrap();
    assert!(patch.contains("-hello") && patch.contains("+bye"));

    let read = |view| Read {
        project: project.clone(),
        view,
        path: "a.txt".into(),
        offset: None,
        limit: None,
        encoding: None,
        anchor: None,
    };
    assert_eq!(rig.views.read(read(Some(view.id.clone()))).await.unwrap().content.unwrap(), "bye\n");
    assert_eq!(rig.views.read(read(None)).await.unwrap().content.unwrap(), "hello\n");
}

#[tokio::test]
async fn worker_views_link_big_binaries_and_copy_text() {
    let rig = Rig::new(1 << 20);
    let blob = format!("0123{}\n", "x".repeat(SHARE_FROM as usize));
    let opened = import(&rig, &[("big.bin", &blob), ("main.rs", "fn main() {}\n")]).await;
    let project = opened.project.id.clone();
    let view = worker(&rig, &project).await;
    let source = view.path.join("main.rs");
    std::fs::OpenOptions::new().write(true).open(&source).unwrap();
    let file = view.path.join("big.bin");
    assert!(std::fs::metadata(&file).unwrap().permissions().readonly());

    rig.views.patch(edit(&project, &view, "big.bin", "0123", "XXXX")).await.unwrap();
    assert!(!std::fs::metadata(&file).unwrap().permissions().readonly());
    assert!(std::fs::read_to_string(&file).unwrap().starts_with("XXXXxx"));
    let entry = rig.hub.store.snapshot(&opened.project.main).unwrap().entries[0].clone();
    let store_copy = rig.dir.path().join("links").join(entry.hash.as_str());
    assert!(std::fs::read_to_string(store_copy).unwrap().starts_with("0123xx"));
}

#[tokio::test]
async fn advance_updates_untouched_files_and_skips_modified() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a1\n"), ("b.txt", "b1\n")]).await;
    let project = opened.project.id.clone();
    let user = opened.view.unwrap();
    let view = worker(&rig, &project).await;
    rig.views.patch(edit(&project, &view, "a.txt", "a1", "a2")).await.unwrap();
    rig.views.patch(edit(&project, &view, "b.txt", "b1", "b2")).await.unwrap();
    let edits = rig.views.capture(&view.id).await.unwrap();
    let base = rig.hub.store.snapshot(&opened.project.main).unwrap();
    let next = rig.hub.store.put_snapshot(&with_edits(&base, &edits)).unwrap();

    std::fs::write(user.path.join("b.txt"), "mine\n").unwrap();
    let skipped = rig.views.advance(&user.id, &next).await.unwrap();
    assert_eq!(skipped, ["b.txt"]);
    assert_eq!(std::fs::read_to_string(user.path.join("a.txt")).unwrap(), "a2\n");
    let changes = rig.views.changes(Changes { project, view: user.id }).await.unwrap();
    assert_eq!(statuses(&changes), [("b.txt".to_string(), FileStatus::Modified)]);
}

#[tokio::test]
async fn pull_writes_merged_files_and_moves_their_base() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a1\n"), ("b.txt", "b1\n")]).await;
    let project = opened.project.id.clone();
    let view = worker(&rig, &project).await;
    rig.views.patch(edit(&project, &view, "a.txt", "a1", "mine")).await.unwrap();
    rig.views.capture(&view.id).await.unwrap();
    let store = &rig.hub.store;
    let file = |text: &str| FileState {
        hash: store.put(text.as_bytes()).unwrap(),
        size: text.len() as u64,
        mode: Default::default(),
    };
    let theirs: Edits = [("a.txt", "theirs\n"), ("b.txt", "b2\n")].map(|(p, t)| (p.to_string(), Some(file(t)))).into();
    let next = store.put_snapshot(&with_edits(&store.snapshot(&opened.project.main).unwrap(), &theirs)).unwrap();
    let merged: Edits = [("a.txt".to_string(), Some(file("merged\n")))].into();

    let pulled = rig.views.pull(&view.id, &next, &merged).await.unwrap();

    assert_eq!(pulled.written, ["a.txt", "b.txt"]);
    assert_eq!(std::fs::read_to_string(view.path.join("a.txt")).unwrap(), "merged\n");
    assert_eq!(std::fs::read_to_string(view.path.join("b.txt")).unwrap(), "b2\n");
    assert_eq!(rig.views.get(&view.id).await.unwrap().base, next);
    let changes = rig.views.changes(Changes { project, view: view.id }).await.unwrap();
    assert_eq!(statuses(&changes), [("a.txt".to_string(), FileStatus::Modified)]);
}

#[tokio::test]
async fn landed_worker_view_is_kept_then_purged() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a\n")]).await;
    let project = opened.project.id.clone();
    let view = worker(&rig, &project).await;
    let main = opened.project.main.clone();
    let change = Change {
        id: ChangeId::new(),
        project: project.clone(),
        parent: main.clone(),
        snapshot: main,
        files: Default::default(),
        author: "agent".into(),
        intent: "test".into(),
        at: now_ms(),
        pinned: false,
        undoes: None,
    };
    rig.projects.append(vec![change.clone()]).await.unwrap();
    rig.views.landed(&view.id, Some(&change.id)).await.unwrap();
    assert_eq!(rig.views.get(&view.id).await.unwrap().state, ViewState::Landed);
    let aside = view.path.with_extension("landed");
    assert!(!view.path.exists() && aside.exists());
    let read = Read {
        project: project.clone(),
        view: Some(view.id.clone()),
        path: "a.txt".into(),
        offset: None,
        limit: None,
        encoding: None,
        anchor: None,
    };
    assert!(rig.views.read(read).await.unwrap_err().message.contains("landed"));

    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(rig.inner.purge().await.unwrap(), 1);
    assert!(!view.path.exists() && !aside.exists());
    assert!(rig.views.get(&view.id).await.is_err());
}

fn spare_id(rig: &Rig) -> String {
    let views = std::fs::read_dir(rig.setup.home.views()).unwrap().flatten().map(|e| e.path());
    let spares: Vec<String> = views
        .filter(|p| p.extension().is_some_and(|e| e == "spare"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(spares.len(), 1);
    spares[0].clone()
}

#[tokio::test]
async fn spare_folder_becomes_the_next_view_and_survives_a_restart() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a\n"), ("sub/b.txt", "b\n")]).await;
    let project = opened.project.id.clone();
    rig.inner.clone().refill(project.clone()).await.unwrap();
    let spare = spare_id(&rig);
    let view = worker(&rig, &project).await;
    assert_eq!(view.id.as_str(), spare);
    assert_eq!(std::fs::read_to_string(view.path.join("sub/b.txt")).unwrap(), "b\n");
    assert!(rig.views.changes(Changes { project: project.clone(), view: view.id }).await.unwrap().files.is_empty());

    rig.inner.clone().refill(project.clone()).await.unwrap();
    let next = spare_id(&rig);
    let restarted = Inner::new(&rig.setup).unwrap();
    let folder = restarted.folder_at(&project, &opened.project.main).await.unwrap();
    assert_eq!(folder.id.as_str(), next);
    assert_eq!(std::fs::read_to_string(folder.root.join("a.txt")).unwrap(), "a\n");
}

#[tokio::test]
async fn watcher_reports_outside_edits() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a\n")]).await;
    let user = opened.view.unwrap();
    let mut events = rig.hub.events.subscribe();
    rig.views.start().unwrap();
    std::fs::write(user.path.join("outside.txt"), "hi\n").unwrap();
    let seen = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Event::ViewChanged { paths, .. } = events.recv().await.unwrap().event
                && paths.contains(&"outside.txt".to_string())
            {
                return;
            }
        }
    })
    .await;
    assert!(seen.is_ok());
}

#[tokio::test]
async fn linked_outputs_follow_the_latest_install_and_stay_out_of_changes() {
    let rig = Rig::new(1 << 20);
    let manifest = r#"[targets.install]
outputs = ["node_modules/**"]
scratch = ["node_modules/.cache/**"]
link_outputs = true
"#;
    let opened = import(&rig, &[("orrin.toml", manifest), ("package.json", "{}")]).await;
    let project = opened.project.id.clone();
    let view = worker(&rig, &project).await;
    let install = |files: &[(&str, &str)]| {
        let entries = files.iter().map(|(path, body)| Entry {
            path: path.to_string(),
            hash: rig.hub.store.put(body.as_bytes()).unwrap(),
            size: body.len() as u64,
            mode: Default::default(),
        });
        let files = rig.hub.store.put_snapshot(&Snapshot::new(entries.collect())).unwrap();
        Linked { target: "install".into(), files }
    };
    let first = install(&[("node_modules/a/index.js", "a1"), ("node_modules/old.js", "x")]);
    rig.views.link(&project, first).await.unwrap();
    std::fs::create_dir_all(view.path.join("node_modules/.cache")).unwrap();
    std::fs::write(view.path.join("node_modules/.cache/hit"), "c").unwrap();
    rig.views.link(&project, install(&[("node_modules/a/index.js", "a2")])).await.unwrap();
    assert_eq!(std::fs::read_to_string(view.path.join("node_modules/a/index.js")).unwrap(), "a2");
    assert!(!view.path.join("node_modules/old.js").exists());
    assert!(view.path.join("node_modules/.cache/hit").exists());
    assert!(rig.views.changes(Changes { project, view: view.id }).await.unwrap().files.is_empty());
}

#[tokio::test]
async fn delivered_outputs_are_real_files_with_their_mtimes_and_stay_out_of_changes() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "a\n")]).await;
    let (project, user) = (opened.project.id.clone(), opened.view.unwrap());
    let built = UNIX_EPOCH + Duration::from_millis(1_600_000_000_000);
    let file = |body: &str| Artifact {
        path: "bin/x.dll".into(),
        hash: rig.hub.store.put(body.as_bytes()).unwrap(),
        size: body.len() as u64,
        modified: Some(1_600_000_000_000),
    };
    let first = rig.views.deliver(&project, vec![file("v1")]).await.unwrap();
    assert_eq!((first.copied, first.unchanged), (1, 0));
    let dll = user.path.join("bin/x.dll");
    let meta = std::fs::metadata(&dll).unwrap();
    assert!(!meta.permissions().readonly());
    assert_eq!(meta.modified().unwrap(), built);
    let changes = rig.views.changes(Changes { project: project.clone(), view: user.id.clone() }).await.unwrap();
    assert!(changes.files.is_empty());
    let again = rig.views.deliver(&project, vec![file("v1")]).await.unwrap();
    assert_eq!((again.copied, again.unchanged), (0, 1));

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&dll).unwrap();
        let busy = rig.views.deliver(&project, vec![file("v2")]).await.unwrap();
        assert_eq!(busy.busy, ["bin/x.dll"]);
        drop(held);
    }
    let done = rig.views.deliver(&project, vec![file("v2")]).await.unwrap();
    assert_eq!((done.copied, done.busy.len()), (1, 0));
    assert_eq!(std::fs::read_to_string(&dll).unwrap(), "v2");
}

#[tokio::test]
async fn removing_a_project_deletes_worker_folders_but_keeps_the_user_folder() {
    let rig = Rig::new(1 << 20);
    let opened = import(&rig, &[("a.txt", "hello\n")]).await;
    let project = opened.project.id.clone();
    let user = opened.view.unwrap();
    let agent = worker(&rig, &project).await;

    assert_eq!(rig.views.remove_project(&project).await.unwrap(), 2);
    assert!(!agent.path.exists());
    assert_eq!(std::fs::read_to_string(user.path.join("a.txt")).unwrap(), "hello\n");
    assert!(rig.views.list(ViewList { project: Some(project) }).await.unwrap().views.is_empty());
    assert!(rig.views.get(&user.id).await.is_err());
}
