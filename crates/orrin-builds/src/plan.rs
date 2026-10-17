use std::sync::Arc;

use orrin_core::model::{Job, JobKind, JobState, Manifest, Snapshot, Target};
use orrin_core::service::Store;
use orrin_core::{JobId, PcId, ProjectId, Result, SnapshotId, now_ms};

use crate::manifest;

pub struct Plan {
    pub kind: JobKind,
    pub project: ProjectId,
    pub snapshot_id: SnapshotId,
    pub snapshot: Snapshot,
    pub manifest: Arc<Manifest>,
    pub name: String,
    pub target: Target,
    pub args: Vec<String>,
}

pub struct Request {
    pub kind: JobKind,
    pub project: ProjectId,
    pub snapshot_id: SnapshotId,
    pub snapshot: Snapshot,
    pub target: String,
    pub args: Vec<String>,
}

impl Plan {
    pub fn load(store: &dyn Store, request: Request) -> Result<Self> {
        let manifest = manifest::load(store, &request.snapshot)?;
        Self::with(Arc::new(manifest), request)
    }

    fn with(manifest: Arc<Manifest>, request: Request) -> Result<Self> {
        manifest::check_after(&manifest, &request.target)?;
        let target = manifest::target(&manifest, &request.target)?.clone();
        Ok(Self {
            kind: target.kind.unwrap_or(request.kind),
            project: request.project,
            snapshot_id: request.snapshot_id,
            snapshot: request.snapshot,
            manifest,
            name: request.target,
            target,
            args: request.args,
        })
    }

    pub fn sibling(&self, name: &str) -> Result<Self> {
        let request = Request {
            kind: JobKind::Build,
            project: self.project.clone(),
            snapshot_id: self.snapshot_id.clone(),
            snapshot: self.snapshot.clone(),
            target: name.to_string(),
            args: Vec::new(),
        };
        Self::with(self.manifest.clone(), request)
    }

    pub fn cacheable(&self) -> bool {
        self.kind != JobKind::Run
    }

    pub fn cpu(&self, cores: u32) -> u32 {
        self.target.cpu.unwrap_or(if self.target.steps.is_empty() { 1 } else { cores })
    }

    pub fn needs_gpu(&self) -> bool {
        self.target.gpu || self.target.needs.iter().any(|n| n == "gpu")
    }

    pub fn tools(&self) -> Vec<String> {
        tool_names(&self.target)
    }

    pub fn job(&self, pc: PcId) -> Job {
        Job {
            id: JobId::new(),
            project: self.project.clone(),
            kind: self.kind,
            target: self.name.clone(),
            snapshot: self.snapshot_id.clone(),
            pc,
            state: JobState::Queued,
            cached: false,
            exit_code: None,
            error: None,
            created_at: now_ms(),
            started_at: None,
            finished_at: None,
            result: None,
        }
    }
}

pub fn tool_names(target: &Target) -> Vec<String> {
    target.needs.iter().filter(|n| *n != "gpu").cloned().collect()
}
