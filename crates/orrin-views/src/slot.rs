use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use notify::RecommendedWatcher;
use orrin_core::model::{View, ViewKind, ViewState};
use orrin_core::service::Edits;
use orrin_core::{Millis, ProjectId, ViewId};
use serde::{Deserialize, Serialize};

use crate::disk::rules::Rules;
use crate::disk::stats::Stats;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewRecord {
    pub view: View,
    #[serde(default)]
    pub captured: Edits,
    #[serde(default)]
    pub closed_at: Option<Millis>,
}

pub struct Slot {
    pub id: ViewId,
    pub project: ProjectId,
    pub kind: ViewKind,
    pub root: PathBuf,
    pub record: tokio::sync::Mutex<ViewRecord>,
    stats: Mutex<Stats>,
    rules: Mutex<Arc<Rules>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

impl Slot {
    pub fn new(record: ViewRecord, stats: Stats) -> Arc<Self> {
        let view = &record.view;
        Arc::new(Self {
            id: view.id.clone(),
            project: view.project.clone(),
            kind: view.kind,
            root: view.path.clone(),
            rules: Mutex::new(Arc::new(Rules::load(&view.path, &[]))),
            record: tokio::sync::Mutex::new(record),
            stats: Mutex::new(stats),
            watcher: Mutex::new(None),
        })
    }

    pub fn stats(&self) -> MutexGuard<'_, Stats> {
        self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn rules(&self) -> Arc<Rules> {
        self.rules.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_rules(&self, rules: Rules) -> Arc<Rules> {
        let rules = Arc::new(rules);
        *self.rules.lock().unwrap_or_else(|e| e.into_inner()) = rules.clone();
        rules
    }

    pub fn set_watcher(&self, watcher: Option<RecommendedWatcher>) {
        *self.watcher.lock().unwrap_or_else(|e| e.into_inner()) = watcher;
    }

    pub fn is_open(record: &ViewRecord) -> bool {
        record.view.state == ViewState::Open
    }
}
