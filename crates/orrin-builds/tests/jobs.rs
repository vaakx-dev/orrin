mod fakes;

use std::time::{Duration, Instant};

use orrin_core::api::{JobLogs, JobRef};
use orrin_core::model::{JobKind, JobState};
use orrin_core::service::Store;

use fakes::{World, world};

#[cfg(windows)]
mod cmd {
    pub const WRITE: &str = "echo built> out.txt";
    pub const WHERE: &str = "cd > where.txt & type a.txt > b.txt";
    pub const SLEEP: &str = "ping -n 30 127.0.0.1 > nul";
    pub const NAP: &str = "ping -n 3 127.0.0.1 > nul & echo done> out.txt";
    pub const LIB: &str = "echo built> lib.txt";
    pub const OTHER: &str = "echo other> other.txt";
    pub const APP: &str = "type lib.txt > app.out & if exist other.txt echo leaked> leak.out";
    pub const WARM: &str = "echo {recipe}>> stamp.out";
    pub const LIB_OBJ: &str = "mkdir obj & echo obj> obj\\x.o & echo built> lib.txt";
    pub const USE_OBJ: &str = "type obj\\x.o > app.out";
}

#[cfg(not(windows))]
mod cmd {
    pub const WRITE: &str = "echo built > out.txt";
    pub const WHERE: &str = "pwd > where.txt; cat a.txt > b.txt";
    pub const SLEEP: &str = "sleep 30";
    pub const NAP: &str = "sleep 2; echo done > out.txt";
    pub const LIB: &str = "echo built > lib.txt";
    pub const OTHER: &str = "echo other > other.txt";
    pub const APP: &str = "cat lib.txt > app.out; if [ -f other.txt ]; then echo leaked > leak.out; fi";
    pub const WARM: &str = "echo {recipe} >> stamp.out";
    pub const LIB_OBJ: &str = "mkdir -p obj; echo obj > obj/x.o; echo built > lib.txt";
    pub const USE_OBJ: &str = "cat obj/x.o > app.out";
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
    let logs = w.builds.logs(JobLogs { job: job.id, after: None, limit: None, wait_ms: None }).await.unwrap();
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

async fn outputs(w: &World, kind: JobKind, recipe: &str) -> Vec<(String, String)> {
    let job = w.builds.submit(kind, w.input(recipe)).await.unwrap();
    let job = w.finished(&job.id).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    let artifacts = w.builds.artifacts(JobRef { job: job.id }).await.unwrap().artifacts;
    artifacts.into_iter().map(|a| (a.path, String::from_utf8(w.store.get(&a.hash).unwrap()).unwrap())).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn after_brings_only_its_outputs() {
    let manifest = format!(
        "[recipes.lib]\ncommand = '{}'\noutputs = [\"*.txt\"]\n\
         [recipes.other]\ncommand = '{}'\noutputs = [\"*.txt\"]\n\
         [recipes.app]\ncommand = '{}'\noutputs = [\"*.out\"]\nafter = [\"lib\"]\n",
        cmd::LIB,
        cmd::OTHER,
        cmd::APP
    );
    let w = world(&[("orrin.toml", &manifest)]);
    outputs(&w, JobKind::Build, "other").await;
    let built = outputs(&w, JobKind::Build, "app").await;
    assert_eq!(built.len(), 1, "{built:?}");
    assert_eq!(built[0].0, "app.out");
    assert_eq!(built[0].1.trim(), "built");
    let app = job_of(&w, "app").await;
    assert!(log_text(&w, &app).await.iter().any(|line| line.starts_with("[lib] $ ")));
}

#[tokio::test(flavor = "multi_thread")]
async fn share_carries_build_files_from_the_after_warm_folder() {
    let manifest = format!(
        "[recipes.lib]\ncommand = '{}'\noutputs = [\"lib.txt\"]\nwarm = true\n\
         [recipes.app]\ncommand = '{}'\noutputs = [\"*.out\"]\nafter = [\"lib\"]\nwarm = 2\nshare = [\"obj/**\"]\n",
        cmd::LIB_OBJ,
        cmd::USE_OBJ
    );
    let w = world(&[("orrin.toml", &manifest)]);
    let built = outputs(&w, JobKind::Build, "app").await;
    assert_eq!(built[0].1.trim(), "obj", "{built:?}");
    let logs = log_text(&w, &job_of(&w, "app").await).await;
    assert!(logs.iter().any(|line| line == "share from lib: 1 copied, 0 already in place"), "{logs:?}");
}

async fn job_of(w: &World, recipe: &str) -> orrin_core::JobId {
    let jobs = w.builds.list(Default::default()).await.unwrap().jobs;
    jobs.into_iter().rev().find(|job| job.recipe == recipe).unwrap().id
}

async fn log_text(w: &World, job: &orrin_core::JobId) -> Vec<String> {
    let ask = JobLogs { job: job.clone(), after: None, limit: None, wait_ms: Some(10) };
    w.builds.logs(ask).await.unwrap().lines.into_iter().map(|line| line.text).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn warm_folders_keep_build_state_between_runs() {
    let manifest = format!("[recipes.warm]\ncommand = '{}'\noutputs = [\"*.out\"]\nwarm = true\n", cmd::WARM);
    let w = world(&[("orrin.toml", &manifest)]);
    outputs(&w, JobKind::Run, "warm").await;
    let second = outputs(&w, JobKind::Run, "warm").await;
    let lines: Vec<&str> = second[0].1.lines().map(str::trim).collect();
    assert_eq!(lines, ["warm", "warm"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn finished_jobs_survive_a_restart() {
    let manifest = format!("[recipes.lib]\ncommand = '{}'\noutputs = [\"*.txt\"]\n", cmd::LIB);
    let w = world(&[("orrin.toml", &manifest)]);
    let job = w.builds.submit(JobKind::Build, w.input("lib")).await.unwrap();
    let job = w.finished(&job.id).await;
    let before = w.builds.artifacts(JobRef { job: job.id.clone() }).await.unwrap().artifacts;

    let builds = w.restart();
    assert_eq!(builds.status(JobRef { job: job.id.clone() }).await.unwrap(), job);
    let logs = builds.logs(JobLogs { job: job.id.clone(), after: None, limit: None, wait_ms: None }).await.unwrap();
    assert!(logs.done && logs.lines[0].text.starts_with("$ "));
    assert_eq!(builds.artifacts(JobRef { job: job.id }).await.unwrap().artifacts, before);
}
