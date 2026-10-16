mod fakes;

use std::collections::BTreeMap;
use std::time::Duration;

use fakes::Node;
use orrin_core::api::{Log, PcsList, PcsPair, PcsRedeem, PcsSettings};
use orrin_core::model::{Change, Entry, Mode, Project, ProjectSettings, Snapshot};
use orrin_core::{ChangeId, Hash, ProjectId, now_ms};

async fn seed(node: &Node) -> (Project, Hash) {
    let store = &node.hub.store;
    let file = store.put(b"hello from a").unwrap();
    let empty = store.put_snapshot(&Snapshot::default()).unwrap();
    let entry = Entry { path: "a.txt".into(), hash: file.clone(), size: 12, mode: Mode::File };
    let main = store.put_snapshot(&Snapshot::new(vec![entry.clone()])).unwrap();
    let project = Project {
        id: ProjectId::new(),
        name: "demo".into(),
        home_pc: node.hub.me.pc.clone(),
        main: main.clone(),
        settings: ProjectSettings::default(),
        created_at: now_ms(),
    };
    let change = Change {
        id: ChangeId::new(),
        project: project.id.clone(),
        parent: empty,
        snapshot: main,
        files: BTreeMap::from([("a.txt".to_string(), Some(entry.state()))]),
        author: "me".into(),
        intent: "add a".into(),
        at: now_ms(),
        pinned: false,
        undoes: None,
    };
    node.hub.projects.save(project.clone()).await.unwrap();
    node.hub.projects.append(vec![change]).await.unwrap();
    (project, file)
}

fn log(project: &ProjectId) -> Log {
    Log { project: project.clone(), path: None, before: None, limit: None }
}

#[tokio::test(flavor = "multi_thread")]
async fn pairs_and_follows_a_project() {
    let a = Node::start("a").await;
    let b = Node::start("b").await;
    let (project, file) = seed(&a).await;

    let code = a.hub.peers.pair(PcsPair {}).await.unwrap().code;
    let paired = b.hub.peers.redeem(PcsRedeem { code: code.clone(), address: String::new() }).await.unwrap();
    assert_eq!(paired.id, a.hub.me.pc);
    assert!(b.hub.peers.redeem(PcsRedeem { code, address: String::new() }).await.is_err());
    let listed = a.hub.peers.pcs(PcsList {}).await.unwrap().pcs;
    assert!(listed.iter().any(|pc| pc.id == b.hub.me.pc && !pc.me));
    assert!(a.home.root().join("peers.json").exists() && b.home.root().join("peers.json").exists());
    assert!(!a.hub.peers.authorize(&b.hub.me.pc, "wrong").await);
    assert!(b.hub.peers.settings(PcsSettings::default()).await.unwrap().builds);

    b.hub.peers.start().unwrap();
    let mut followed = None;
    for _ in 0..50 {
        if let Ok(copy) = b.hub.projects.get(&project.id).await {
            followed = Some(copy);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(followed.unwrap().main, project.main);
    assert_eq!(b.hub.store.get(&file).unwrap(), b"hello from a");
    assert_eq!(b.hub.projects.log(log(&project.id)).await.unwrap().changes.len(), 1);
}

async fn land(node: &Node, project: &Project, text: &[u8]) -> Hash {
    let store = &node.hub.store;
    let file = store.put(text).unwrap();
    let entry = Entry { path: "a.txt".into(), hash: file, size: text.len() as u64, mode: Mode::File };
    let main = store.put_snapshot(&Snapshot::new(vec![entry.clone()])).unwrap();
    let change = Change {
        id: ChangeId::new(),
        project: project.id.clone(),
        parent: project.main.clone(),
        snapshot: main.clone(),
        files: BTreeMap::from([("a.txt".to_string(), Some(entry.state()))]),
        author: "me".into(),
        intent: "edit a".into(),
        at: now_ms(),
        pinned: false,
        undoes: None,
    };
    node.hub.projects.swap_main(change).await.unwrap();
    main
}

async fn main_of(node: &Node, project: &ProjectId) -> Option<Hash> {
    node.hub.projects.get(project).await.ok().map(|p| p.main)
}

#[tokio::test(flavor = "multi_thread")]
async fn pokes_followers_when_main_moves() {
    let a = Node::start("a").await;
    let b = Node::start("b").await;
    let (project, _) = seed(&a).await;
    let code = a.hub.peers.pair(PcsPair {}).await.unwrap().code;
    b.hub.peers.redeem(PcsRedeem { code, address: String::new() }).await.unwrap();
    a.hub.peers.start().unwrap();
    b.hub.peers.start().unwrap();
    for _ in 0..250 {
        if main_of(&b, &project.id).await == Some(project.main.clone()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let started = std::time::Instant::now();
    let main = land(&a, &project, b"hello again").await;
    while main_of(&b, &project.id).await != Some(main.clone()) {
        assert!(started.elapsed() < Duration::from_secs(1), "b did not follow within 1 s");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    println!("b followed in {:?}", started.elapsed());
}
