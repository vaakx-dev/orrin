use std::time::Duration;

use orrin_core::api::*;
use orrin_core::model::{Change, FileStatus, SHARE_FROM, View, ViewState};
use orrin_core::service::Projects;
use orrin_core::{ChangeId, ProjectId, now_ms};

use crate::snapshots::with_edits;
use crate::tests::fakes::Rig;

async fn import(rig: &Rig, files: &[(&str, &str)]) -> ProjectOpened {
    let path = rig.folder(files);
    rig.views.import(ProjectsImport { path, name: None }).await.unwrap()
}

async fn worker(rig: &Rig, project: &ProjectId) -> View {
    rig.views
        .create(ViewCreate { project: project.clone(), owner: "agent".into(), base: None, claims: vec![] })
        .await
        .unwrap()
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
    ];
    let opened = import(&rig, &files).await;
    let main = rig.hub.store.snapshot(&opened.project.main).unwrap();
    let paths: Vec<&str> = main.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, [".gitignore", "a.txt", "orrin.toml", "sub/b.txt"]);
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
    rig.views.landed(&view.id, &change.id).await.unwrap();
    assert_eq!(rig.views.get(&view.id).await.unwrap().state, ViewState::Landed);
    assert!(view.path.exists());

    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(rig.inner.purge().await.unwrap(), 1);
    assert!(!view.path.exists());
    assert!(rig.views.get(&view.id).await.is_err());
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
