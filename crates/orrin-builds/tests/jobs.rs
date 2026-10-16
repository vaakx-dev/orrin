mod fakes;

use std::time::{Duration, Instant};

use orrin_core::api::{JobLogs, JobRef};
use orrin_core::model::{JobKind, JobState};
use orrin_core::service::Store;

use fakes::world;

#[cfg(windows)]
mod cmd {
    pub const WRITE: &str = "echo built> out.txt";
    pub const WHERE: &str = "cd > where.txt & type a.txt > b.txt";
    pub const SLEEP: &str = "ping -n 30 127.0.0.1 > nul";
    pub const NAP: &str = "ping -n 3 127.0.0.1 > nul & echo done> out.txt";
}

#[cfg(not(windows))]
mod cmd {
    pub const WRITE: &str = "echo built > out.txt";
    pub const WHERE: &str = "pwd > where.txt; cat a.txt > b.txt";
    pub const SLEEP: &str = "sleep 30";
    pub const NAP: &str = "sleep 2; echo done > out.txt";
}

fn manifest(name: &str, command: &str) -> String {
    format!("[recipes.{name}]\ncommand = '{command}'\noutputs = [\"*.txt\"]\ncpu = 1\nram_mb = 256\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn second_build_is_a_cache_hit() {
    let manifest = manifest("app", cmd::WRITE);
    let w = world(&[("orrin.toml", &manifest)]);
    let first = w.builds.submit(JobKind::Build, w.input("app")).await.unwrap();
    let first = w.finished(&first.id).await;
    assert_eq!(first.state, JobState::Succeeded, "{first:?}");
    let artifacts = w.builds.artifacts(JobRef { job: first.id.clone() }).await.unwrap().artifacts;
    assert_eq!(artifacts.len(), 1);
    assert!(w.store.has(&artifacts[0].hash));

    let second = w.builds.submit(JobKind::Build, w.input("app")).await.unwrap();
    assert_eq!(second.state, JobState::Succeeded);
    assert!(second.cached);
    assert_eq!(w.builds.artifacts(JobRef { job: second.id }).await.unwrap().artifacts, artifacts);
}

#[tokio::test(flavor = "multi_thread")]
async fn identical_running_jobs_are_shared() {
    let manifest = manifest("slow", cmd::NAP);
    let w = world(&[("orrin.toml", &manifest)]);
    let first = w.builds.submit(JobKind::Build, w.input("slow")).await.unwrap();
    let second = w.builds.submit(JobKind::Build, w.input("slow")).await.unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(w.finished(&first.id).await.state, JobState::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_in_a_fresh_folder_from_the_snapshot() {
    let manifest = manifest("where", cmd::WHERE);
    let w = world(&[("orrin.toml", &manifest), ("a.txt", "from the snapshot")]);
    let job = w.builds.submit(JobKind::Test, w.input("where")).await.unwrap();
    let job = w.finished(&job.id).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    let artifacts = w.builds.artifacts(JobRef { job: job.id.clone() }).await.unwrap().artifacts;
    let get = |path: &str| {
        let artifact = artifacts.iter().find(|a| a.path == path).unwrap();
        String::from_utf8(w.store.get(&artifact.hash).unwrap()).unwrap()
    };
    let run_dir = w.home.runs().join(job.id.as_str());
    assert_eq!(get("where.txt").trim().to_lowercase(), run_dir.to_string_lossy().to_lowercase());
    assert_eq!(get("b.txt").trim(), "from the snapshot");
    assert!(!artifacts.iter().any(|a| a.path == "a.txt"));
    assert!(!run_dir.exists());
    let logs = w.builds.logs(JobLogs { job: job.id, after: None, limit: None }).await.unwrap();
    assert!(logs.done && logs.lines[0].text.starts_with("$ "));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_kills_the_job() {
    let manifest = manifest("forever", cmd::SLEEP);
    let w = world(&[("orrin.toml", &manifest)]);
    let job = w.builds.submit(JobKind::Run, w.input("forever")).await.unwrap();
    for _ in 0..100 {
        if w.builds.status(JobRef { job: job.id.clone() }).await.unwrap().state == JobState::Running {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let started = Instant::now();
    let cancelled = w.builds.cancel(JobRef { job: job.id }).await.unwrap();
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
}
