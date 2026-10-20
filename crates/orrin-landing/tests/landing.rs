mod fakes;

use fakes::Rig;
use orrin_core::api::{Event, Undo, ViewLand, ViewPull};
use orrin_core::model::{ViewKind, ViewState};

fn land(view: &orrin_core::ViewId) -> ViewLand {
    ViewLand { view: view.clone(), intent: "work".into(), into: None }
}

fn land_into(view: &orrin_core::ViewId, parent: &orrin_core::ViewId) -> ViewLand {
    ViewLand { into: Some(parent.clone()), ..land(view) }
}

#[tokio::test]
async fn lands_into_a_parent_view_and_merges_its_newer_edits() {
    let mut rig = Rig::new(&[("a.txt", "1\n2\n3\n")]);
    let parent = rig.view(ViewKind::Worker, &[("a.txt", Some("P\n2\n3\n"))]);
    let child = rig.child(&parent, &[("a.txt", Some("P\n2\nC\n")), ("b.txt", Some("new\n"))]);
    rig.edit(&parent, &[("a.txt", Some("P\nQ\n3\n"))]);
    let main = rig.main();

    let out = rig.hub.landing.land(land_into(&child, &parent)).await.unwrap();

    assert!(out.landed, "{:?}", out.conflicts);
    assert_eq!(rig.main(), main);
    assert_eq!(rig.read_in(&parent, "a.txt").as_deref(), Some("P\nQ\nC\n"));
    assert_eq!(rig.read_in(&parent, "b.txt").as_deref(), Some("new\n"));
    assert_eq!(rig.views.view(&child).state, ViewState::Landed);
    assert_eq!(rig.views.view(&parent).state, ViewState::Open);
    let into_parent =
        |e: &Event| matches!(e, Event::ViewLanded { view, into: Some(p), .. } if view == &child && p == &parent);
    assert!(rig.events().iter().any(into_parent));
}

#[tokio::test]
async fn landing_into_a_parent_returns_conflicts_and_keeps_the_view_open() {
    let rig = Rig::new(&[("a.txt", "1\n2\n3\n")]);
    let parent = rig.view(ViewKind::Worker, &[]);
    let child = rig.child(&parent, &[("a.txt", Some("1\n2\nC\n"))]);
    rig.edit(&parent, &[("a.txt", Some("1\n2\nP\n"))]);

    let out = rig.hub.landing.land(land_into(&child, &parent)).await.unwrap();

    assert!(!out.landed);
    assert_eq!(out.conflicts.len(), 1);
    assert_eq!(rig.read_in(&parent, "a.txt").as_deref(), Some("1\n2\nP\n"));
    assert_eq!(rig.views.view(&child).state, ViewState::Open);
}

#[tokio::test]
async fn clean_land_moves_main_and_advances_the_user_view() {
    let mut rig = Rig::new(&[("a.txt", "1\n2\n")]);
    let user = rig.view(ViewKind::User, &[]);
    rig.views.busy.lock().unwrap().push("open.txt".into());
    let worker = rig.view(ViewKind::Worker, &[("a.txt", Some("1\nX\n")), ("b.txt", Some("new\n"))]);

    let out = rig.hub.landing.land(land(&worker)).await.unwrap();

    assert!(out.landed);
    assert_eq!(out.main, rig.main());
    assert_eq!(rig.read("a.txt").as_deref(), Some("1\nX\n"));
    assert_eq!(rig.read("b.txt").as_deref(), Some("new\n"));
    assert_eq!(out.skipped, vec!["open.txt".to_string()]);
    assert_eq!(rig.views.view(&worker).state, ViewState::Landed);
    assert_eq!(rig.views.view(&user).base, rig.main());
    let events = rig.events();
    assert!(events.iter().any(|e| matches!(e, Event::ViewLanded { view, .. } if *view == worker)));
}

#[tokio::test]
async fn rebases_onto_a_moved_main() {
    let rig = Rig::new(&[("a.txt", "1\n2\n"), ("b.txt", "b\n")]);
    let worker = rig.view(ViewKind::Worker, &[("a.txt", Some("1\nV\n")), ("b.txt", Some("b2\n"))]);
    rig.move_main(&[("a.txt", Some("M\n2\n")), ("c.txt", Some("c\n"))]).await;

    let out = rig.hub.landing.land(land(&worker)).await.unwrap();

    assert!(out.landed, "{:?}", out.conflicts);
    assert_eq!(rig.read("a.txt").as_deref(), Some("M\nV\n"));
    assert_eq!(rig.read("b.txt").as_deref(), Some("b2\n"));
    assert_eq!(rig.read("c.txt").as_deref(), Some("c\n"));
}

#[tokio::test]
async fn unmergeable_edits_on_both_sides_conflict() {
    let mut rig = Rig::new(&[("hero.png", "base")]);
    let worker = rig.view(ViewKind::Worker, &[("hero.png", Some("ours"))]);
    rig.move_main(&[("hero.png", Some("theirs"))]).await;
    let before = rig.main();

    let out = rig.hub.landing.land(land(&worker)).await.unwrap();

    assert!(!out.landed);
    assert_eq!(rig.main(), before);
    assert_eq!(out.conflicts.len(), 1);
    let conflict = &out.conflicts[0];
    assert_eq!(conflict.path, "hero.png");
    assert!(conflict.base.is_some() && conflict.ours.is_some() && conflict.theirs.is_some());
    assert_eq!(rig.views.view(&worker).state, ViewState::Open);
    assert!(rig.events().iter().any(|e| matches!(e, Event::ViewConflicts { view, .. } if *view == worker)));
}

#[tokio::test]
async fn pull_merges_main_into_edited_files_and_reports_what_it_kept() {
    let rig = Rig::new(&[("a.txt", "1\n2\n"), ("hero.png", "base")]);
    let worker = rig.view(ViewKind::Worker, &[("a.txt", Some("1\nV\n")), ("hero.png", Some("ours"))]);
    rig.move_main(&[("a.txt", Some("M\n2\n")), ("hero.png", Some("theirs"))]).await;

    let out = rig.hub.landing.pull(ViewPull { view: worker.clone(), from: None }).await.unwrap();

    assert_eq!(out.changed, ["a.txt"]);
    assert_eq!(out.conflicts.len(), 1);
    assert!(out.conflicts[0].reason.contains("kept your version"), "{}", out.conflicts[0].reason);
    assert_eq!(rig.read_in(&worker, "a.txt").as_deref(), Some("M\nV\n"));
    assert_eq!(rig.views.view(&worker).base, rig.main());
}

#[tokio::test]
async fn undo_reverts_onto_the_latest_main() {
    let rig = Rig::new(&[("a.txt", "1\n2\n")]);
    let worker = rig.view(ViewKind::Worker, &[("a.txt", Some("1\nX\n")), ("b.txt", Some("new\n"))]);
    let landed = rig.hub.landing.land(land(&worker)).await.unwrap().change.unwrap();
    rig.move_main(&[("a.txt", Some("M\nX\n"))]).await;

    let undo = Undo { project: rig.project.clone(), change: None, author: None };
    let out = rig.hub.landing.undo(undo).await.unwrap();

    assert!(out.landed, "{:?}", out.conflicts);
    assert_eq!(out.change.unwrap().undoes, Some(landed.id));
    assert_eq!(rig.read("a.txt").as_deref(), Some("M\n2\n"));
    assert_eq!(rig.read("b.txt"), None);
}
