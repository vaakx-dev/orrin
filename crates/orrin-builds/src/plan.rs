use std::collections::BTreeMap;

use orrin_core::model::{Job, JobKind, JobState, Recipe, Snapshot};
use orrin_core::service::Store;
use orrin_core::{Error, JobId, PcId, ProjectId, Result, SnapshotId, now_ms};

use crate::recipe;

pub struct Plan {
    pub kind: JobKind,
    pub project: ProjectId,
    pub snapshot_id: SnapshotId,
    pub snapshot: Snapshot,
    pub recipes: BTreeMap<String, Recipe>,
    pub name: String,
    pub recipe: Recipe,
    pub args: Vec<String>,
}

pub struct Target {
    pub kind: JobKind,
    pub project: ProjectId,
    pub snapshot_id: SnapshotId,
    pub snapshot: Snapshot,
    pub recipe: String,
    pub args: Vec<String>,
}

impl Plan {
    pub fn load(store: &dyn Store, target: Target) -> Result<Self> {
        let recipes = recipe::load_all(store, &target.snapshot)?;
        Self::with(recipes, target)
    }

    fn with(recipes: BTreeMap<String, Recipe>, target: Target) -> Result<Self> {
        recipe::check_after(&recipes, &target.recipe)?;
        let recipe = recipes
            .get(&target.recipe)
            .cloned()
            .ok_or_else(|| Error::not_found(format!("no recipe {}", target.recipe)))?;
        Ok(Self {
            kind: target.kind,
            project: target.project,
            snapshot_id: target.snapshot_id,
            snapshot: target.snapshot,
            recipes,
            name: target.recipe,
            recipe,
            args: target.args,
        })
    }

    pub fn sibling(&self, name: &str) -> Result<Self> {
        let target = Target {
            kind: JobKind::Build,
            project: self.project.clone(),
            snapshot_id: self.snapshot_id.clone(),
            snapshot: self.snapshot.clone(),
            recipe: name.to_string(),
            args: Vec::new(),
        };
        Self::with(self.recipes.clone(), target)
    }

    pub fn cacheable(&self) -> bool {
        self.kind != JobKind::Run
    }

    pub fn needs_gpu(&self) -> bool {
        self.recipe.gpu || self.recipe.needs.iter().any(|n| n == "gpu")
    }

    pub fn tools(&self) -> Vec<String> {
        tool_names(&self.recipe)
    }

    pub fn job(&self, pc: PcId) -> Job {
        Job {
            id: JobId::new(),
            project: self.project.clone(),
            kind: self.kind,
            recipe: self.name.clone(),
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

pub fn tool_names(recipe: &Recipe) -> Vec<String> {
    recipe.needs.iter().filter(|n| *n != "gpu").cloned().collect()
}
