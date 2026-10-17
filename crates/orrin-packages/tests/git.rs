use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use orrin_core::model::Package;
use orrin_core::service::{Hub, PackageNeed, Packages, Services, Setup, Unavailable};
use orrin_core::{Config, Events, Home, Identity, ProjectId};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(dir).status().unwrap();
    assert!(status.success(), "git {args:?}");
}

fn repo(dir: &Path) {
    std::fs::create_dir_all(dir.join("include")).unwrap();
    std::fs::write(dir.join("include/lib.h"), "int lib(void);\n").unwrap();
    git(dir, &["init", "--quiet"]);
    git(dir, &["-c", "user.name=t", "-c", "user.email=t@t", "add", "."]);
    git(dir, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--quiet", "-m", "one"]);
    git(dir, &["tag", "v1"]);
}

fn wire(home: &Home, keep_days: u32) -> Arc<dyn Packages> {
    let mut config = Config::default();
    config.packages.keep_days = keep_days;
    let me = Identity::load_or_create(home).unwrap();
    let setup = Setup { home: home.clone(), config, me, events: Events::new(), hub: Hub::new() };
    let packages = orrin_packages::open(&setup).unwrap();
    let none = Arc::new(Unavailable("test"));
    setup.hub.set(Services {
        home: setup.home.clone(),
        config: setup.config.clone(),
        me: setup.me.clone(),
        events: setup.events.clone(),
        store: orrin_store::open(&setup).unwrap(),
        projects: none.clone(),
        views: none.clone(),
        landing: none.clone(),
        mergers: Vec::new(),
        index: none.clone(),
        facts: none.clone(),
        builds: none.clone(),
        cache: none.clone(),
        packages: packages.clone(),
        peers: none.clone(),
        git: none,
    });
    packages
}

#[tokio::test(flavor = "multi_thread")]
async fn fetches_a_tag_once_and_cleans_it_when_unused() {
    let (home_dir, repo_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    repo(repo_dir.path());
    let home = Home::at(home_dir.path()).unwrap();
    let package = Package {
        git: Some(repo_dir.path().to_string_lossy().into_owned()),
        tag: Some("v1".into()),
        ..Default::default()
    };
    let need = PackageNeed { project: ProjectId::new(), name: "lib".into(), package };

    let packages = wire(&home, 0);
    let first = packages.ensure(need.clone()).await.unwrap();
    assert_eq!(first.note, "fetched");
    assert_eq!(std::fs::read_to_string(first.dir.join("include/lib.h")).unwrap(), "int lib(void);\n");
    assert!(!first.dir.join(".git").exists());

    let again = packages.ensure(need).await.unwrap();
    assert_eq!((again.note.as_str(), &again.dir, &again.snapshot), ("ready", &first.dir, &first.snapshot));
    assert!(packages.roots().await.unwrap().contains(&first.snapshot));

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(packages.clean().await.unwrap(), 2);
    assert!(!first.dir.exists());
    assert!(packages.roots().await.unwrap().is_empty());
}
