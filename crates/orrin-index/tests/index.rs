mod fakes;

use orrin_core::api::{Find, Outline, Search, Where};
use orrin_core::{ProjectId, ViewId};

const MAIN_RS: &str = "struct Player;\n\nfn main() {\n    let speed = 3;\n}\n";
const GUIDE: &str = "# Guide\n\nHow to play.\n\n## Controls\n";

fn find(pattern: &str, view: Option<&ViewId>) -> Find {
    Find { project: ProjectId::from("p_test"), view: view.cloned(), pattern: pattern.into(), limit: None }
}

fn search(query: &str, regex: bool, view: Option<&ViewId>) -> Search {
    Search {
        project: ProjectId::from("p_test"),
        view: view.cloned(),
        query: query.into(),
        regex,
        case_sensitive: false,
        paths: None,
        limit: None,
    }
}

fn locate(name: &str) -> Where {
    Where { project: ProjectId::from("p_test"), view: None, name: name.into(), kind: None, limit: None }
}

#[tokio::test]
async fn indexes_main_and_overlays_views() {
    let dir = tempfile::tempdir().unwrap();
    let world =
        fakes::world(dir.path(), &[("src/main.rs", MAIN_RS), ("docs/guide.md", GUIDE), ("art/logo.png", "png")]);
    let index = orrin_index::open(&world.setup).unwrap();
    world.wire(index.clone());
    let project = ProjectId::from("p_test");

    let found = index.find(find("main", None)).await.unwrap();
    assert_eq!(found.files[0].path, "src/main.rs");
    let first_clock = found.clock;

    let hits = index.search(search("SPEED", false, None)).await.unwrap();
    assert_eq!((hits.hits.len(), hits.hits[0].line, hits.hits[0].column), (1, 4, 9));
    let hits = index.search(search(r"fn\s+ma\w+", true, None)).await.unwrap();
    assert_eq!(hits.hits[0].path, "src/main.rs");

    let outline =
        index.outline(Outline { project: project.clone(), view: None, path: "docs/guide.md".into() }).await.unwrap();
    assert_eq!(outline.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["Guide", "Controls"]);

    let player = index.locate(locate("Player")).await.unwrap();
    assert_eq!((player.symbols[0].kind.as_str(), player.symbols[0].line), ("struct", 1));
    assert_eq!(index.locate(locate("blender")).await.unwrap().symbols[0].kind, "tool");
    assert_eq!(index.locate(locate("guide.md")).await.unwrap().symbols[0].path, "docs/guide.md");

    let view = ViewId::from("v_test");
    world.views.write("src/main.rs", Some("fn main() {\n    let velocity = 3;\n}\n"));
    world.views.write("docs/guide.md", None);
    world.views.lock_records(true);
    index.touched(&project, Some(&view), &["src/main.rs".into()]).await.unwrap();
    world.views.lock_records(false);
    assert!(index.search(search("velocity", false, Some(&view))).await.unwrap().hits.len() == 1);
    assert!(index.search(search("velocity", false, None)).await.unwrap().hits.is_empty());
    assert!(index.find(find("guide", Some(&view))).await.unwrap().files.is_empty());
    assert_eq!(index.find(find("guide", None)).await.unwrap().files.len(), 1);

    world.views.write("src/enemy.rs", Some("struct Enemy;\n"));
    let clock = index.touched(&project, Some(&view), &["src/enemy.rs".into()]).await.unwrap();
    assert!(clock > first_clock);
    assert_eq!(index.find(find("enemy", Some(&view))).await.unwrap().clock, clock);
    assert_eq!(index.clock(&project, Some(&view)).await, clock);

    let moved = world.store.snapshot_of(&[("src/main.rs", MAIN_RS), ("src/boss.rs", "struct Boss;\n")]);
    world.project.0.lock().unwrap().main = moved;
    let boss = index.find(find("boss", None)).await.unwrap();
    assert_eq!(boss.files[0].path, "src/boss.rs");
    assert!(boss.clock > clock);
    assert!(index.find(find("guide", None)).await.unwrap().files.is_empty());
    assert!(dir.path().join("index/index.db").exists());
}
