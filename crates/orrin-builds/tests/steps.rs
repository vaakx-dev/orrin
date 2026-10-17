mod fakes;

use std::path::PathBuf;
use std::time::SystemTime;

use orrin_core::JobId;
use orrin_core::api::{JobLogs, JobRef};
use orrin_core::model::{JobKind, JobState};
use orrin_core::service::Store;

use fakes::{World, world};

#[cfg(windows)]
const RULES: &str = "[rules.cc]\ncommand = 'type {in} & copy /y {in} {out} > nul'\ndeps = 'msvc'\noutput = 'obj/{name}.o'\n\
     [rules.link]\ncommand = 'type {in} > {out} 2> nul & echo map> {out.path}.map'\noutputs = ['{out.path}.map']\n";
#[cfg(not(windows))]
const RULES: &str = "[rules.cc]\ncommand = 'cat {in}; cp {in} {out}'\ndeps = 'msvc'\noutput = 'obj/{name}.o'\n\
     [rules.link]\ncommand = 'cat {in} > {out}; echo map > {out.path}.map'\noutputs = ['{out.path}.map']\n";

fn manifest() -> String {
    let steps = "steps = [{ rule = 'cc', files = '*.c' }, { rule = 'link', in = '@previous', out = 'app.out' }]";
    format!("{RULES}[targets.app]\n{steps}\nwarm = true\n[targets.fresh]\n{steps}\n")
}

fn files<'a>(manifest: &'a str, header: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("orrin.toml", manifest),
        ("a.c", "Note: including file: a.h\n"),
        ("b.c", "Note: including file: b.h\n"),
        ("a.h", header),
        ("b.h", "b"),
    ]
}

async fn build(w: &World, target: &str) -> (JobId, Vec<String>, String) {
    let job = w.builds.submit(JobKind::Build, w.input(target)).await.unwrap();
    let job = w.finished(&job.id).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    let ask = JobLogs { job: job.id.clone(), after: None, limit: None, wait_ms: Some(10) };
    let logs = w.builds.logs(ask).await.unwrap().lines.into_iter().map(|line| line.text).collect();
    let artifacts = w.builds.artifacts(JobRef { job: job.id.clone() }).await.unwrap().artifacts;
    assert_eq!(artifacts.len(), 1, "{artifacts:?}");
    assert_eq!(artifacts[0].path, "app.out");
    let out = String::from_utf8(w.store.get(&artifacts[0].hash).unwrap()).unwrap();
    (job.id, logs, out)
}

fn warm(w: &World) -> PathBuf {
    w.home.root().join("warm").join(w.project.as_str()).join("app").join("0")
}

fn modified(w: &World, path: &str) -> SystemTime {
    std::fs::metadata(warm(w).join(path)).unwrap().modified().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn header_change_reruns_only_the_steps_that_include_it() {
    let manifest = manifest();
    let w = world(&files(&manifest, "a"));
    let (_, logs, out) = build(&w, "app").await;
    assert!(logs.contains(&"cc: 2 ran, 0 reused".to_string()), "{logs:?}");
    assert!(logs.contains(&"link: 1 ran, 0 reused".to_string()), "{logs:?}");
    assert!(out.contains("a.h") && out.contains("b.h"), "{out}");
    let untouched = modified(&w, "obj/b.o");
    std::fs::remove_file(warm(&w).join("app.map")).unwrap();

    w.change(&files(&manifest, "a2"));
    let (_, logs, _) = build(&w, "app").await;
    assert!(warm(&w).join("app.map").is_file(), "the link's second output comes back with the set");
    assert!(logs.contains(&"cc: 1 ran, 1 reused".to_string()), "{logs:?}");
    assert!(logs.contains(&"[cc] a.c".to_string()) && !logs.contains(&"[cc] b.c".to_string()), "{logs:?}");
    assert!(logs.contains(&"link: 0 ran, 1 reused".to_string()), "same object, so no relink: {logs:?}");
    assert_eq!(modified(&w, "obj/b.o"), untouched);

    let (_, logs, out) = build(&w, "fresh").await;
    assert!(logs.contains(&"cc: 0 ran, 2 reused".to_string()), "{logs:?}");
    assert!(logs.contains(&"link: 0 ran, 1 reused".to_string()), "{logs:?}");
    assert!(out.contains("a.h") && out.contains("b.h"), "{out}");
}
