mod daemon;

use std::path::Path;
use std::process::Command;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use daemon::Daemon;
use serde_json::{Value, json};

#[cfg(windows)]
const BUILD: &str = "echo lib-built> out.txt";
#[cfg(windows)]
const APP: &str = r"type {pkg:lib}\out.txt {pkg:lib}\include\lib.h > app.txt";
#[cfg(not(windows))]
const BUILD: &str = "echo lib-built > out.txt";
#[cfg(not(windows))]
const APP: &str = "cat {pkg:lib}/out.txt {pkg:lib}/include/lib.h > app.txt";

fn git(dir: &Path, args: &[&str]) {
    let who = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "core.autocrlf=false"];
    let status = Command::new("git").args(who).args(args).current_dir(dir).status().unwrap();
    assert!(status.success(), "git {args:?}");
}

fn library() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("include")).unwrap();
    std::fs::write(dir.path().join("include/lib.h"), "int lib(void);\n").unwrap();
    git(dir.path(), &["init", "--quiet"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    git(dir.path(), &["tag", "v1"]);
    dir
}

fn project(library: &Path, name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("name.txt"), name).unwrap();
    let manifest = format!(
        "[packages.lib]\ngit = '{}'\ntag = 'v1'\nbuild = {{ command = '{BUILD}', outputs = ['out.txt'] }}\n\n\
         [targets.app]\ncommand = '{APP}'\noutputs = ['app.txt']\n",
        library.display()
    );
    std::fs::write(dir.path().join("orrin.toml"), manifest).unwrap();
    dir
}

async fn build(daemon: &Daemon, folder: &Path) -> (String, Vec<String>) {
    let opened = daemon.call("projects.import", json!({ "path": folder })).await;
    let project = opened["result"]["project"]["id"].clone();
    let job = daemon.call("build", json!({ "project": project, "target": "app" })).await;
    let id = job["result"]["id"].clone();
    let mut status = Value::Null;
    for _ in 0..600 {
        status = daemon.call("job.status", json!({ "job": id })).await["result"].clone();
        if ["succeeded", "failed", "cancelled"].contains(&status["state"].as_str().unwrap_or("")) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let logs = daemon.call("job.logs", json!({ "job": id })).await["result"]["lines"].clone();
    let lines: Vec<String> = logs.as_array().unwrap().iter().map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(status["state"], "succeeded", "{status} {lines:#?}");
    let artifacts = daemon.call("job.artifacts", json!({ "job": id })).await["result"]["artifacts"].clone();
    let hash = artifacts.as_array().unwrap().iter().find(|a| a["path"] == "app.txt").unwrap()["hash"].clone();
    let object = daemon.call("objects.get", json!({ "hash": hash })).await;
    let bytes = STANDARD.decode(object["result"]["data"].as_str().unwrap()).unwrap();
    (String::from_utf8(bytes).unwrap(), lines)
}

#[tokio::test(flavor = "multi_thread")]
async fn builds_a_git_package_once_for_every_project() {
    let daemon = Daemon::start().await;
    let library = library();
    let (first, second) = (project(library.path(), "one"), project(library.path(), "two"));

    let (text, lines) = build(&daemon, first.path()).await;
    assert!(text.contains("lib-built") && text.contains("int lib(void);"), "{text}");
    assert!(lines.iter().any(|l| l.starts_with("package lib: built in job")), "{lines:#?}");

    let (again, lines) = build(&daemon, second.path()).await;
    assert_eq!(again, text);
    assert!(lines.iter().any(|l| l.starts_with("package lib: ready")), "{lines:#?}");
    daemon.stop().await;
}
