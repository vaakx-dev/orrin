use std::collections::BTreeMap;

use orrin_core::api::{JobInput, JobLogs, JobRef};
use orrin_core::model::{Entry, Job, JobKind, JobState, MANIFEST_PATH, Manifest, PackageBuild, Snapshot, Target};
use orrin_core::service::{PackageNeed, Services};
use orrin_core::{Error, Result, SnapshotId};

const PAGE: u32 = 1000;
const WAIT_MS: u64 = 25_000;

pub struct Built {
    pub snapshot: SnapshotId,
    pub job: Job,
}

pub async fn run(hub: &Services, need: &PackageNeed, source: &SnapshotId, build: &PackageBuild) -> Result<Built> {
    let name = need.name.as_str();
    let source = hub.store.snapshot(source)?;
    let (snapshot, target) = match build {
        PackageBuild::Target(target) => (source, target.clone()),
        PackageBuild::Inline(inline) => (overlay(hub, source, need, inline)?, name.to_string()),
    };
    let id = hub.store.put_snapshot(&snapshot)?;
    let project = need.project.clone();
    let input = JobInput { project, view: None, snapshot: Some(id), target, args: Vec::new(), pc: None };
    let job = finished(hub, hub.builds.submit(JobKind::Build, input).await?).await?;
    if job.state != JobState::Succeeded {
        let why = job.error.clone().unwrap_or_else(|| format!("{:?}", job.state).to_lowercase());
        return Err(Error::invalid(format!("package {name}: build job {} failed: {why}", job.id)));
    }
    let artifacts = hub.builds.artifacts(JobRef { job: job.id.clone() }).await?.artifacts;
    let mut files: BTreeMap<String, Entry> = snapshot.entries.into_iter().map(|e| (e.path.clone(), e)).collect();
    for artifact in artifacts {
        let entry =
            Entry { path: artifact.path.clone(), hash: artifact.hash, size: artifact.size, mode: Default::default() };
        files.insert(artifact.path, entry);
    }
    let snapshot = hub.store.put_snapshot(&Snapshot::new(files.into_values().collect()))?;
    Ok(Built { snapshot, job })
}

fn overlay(hub: &Services, source: Snapshot, need: &PackageNeed, inline: &Target) -> Result<Snapshot> {
    let name = need.name.as_str();
    let manifest = Manifest {
        targets: BTreeMap::from([(name.to_string(), inline.clone())]),
        rules: need.rules.clone(),
        ..Default::default()
    };
    let manifest =
        toml::to_string(&manifest).map_err(|err| Error::invalid(format!("package {name}: bad build table: {err}")))?;
    let entry = Entry {
        path: MANIFEST_PATH.to_string(),
        hash: hub.store.put(manifest.as_bytes())?,
        size: manifest.len() as u64,
        mode: Default::default(),
    };
    let mut entries: Vec<Entry> = source.entries.into_iter().filter(|e| e.path != MANIFEST_PATH).collect();
    entries.push(entry);
    Ok(Snapshot::new(entries))
}

async fn finished(hub: &Services, job: Job) -> Result<Job> {
    let mut after = 0;
    loop {
        let ask = JobLogs { job: job.id.clone(), after: Some(after), limit: Some(PAGE), wait_ms: Some(WAIT_MS) };
        let page = hub.builds.logs(ask).await?;
        if let Some(last) = page.lines.last() {
            after = last.n;
        }
        if page.done {
            return hub.builds.status(JobRef { job: job.id }).await;
        }
    }
}
