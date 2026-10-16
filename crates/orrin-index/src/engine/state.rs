use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use orrin_core::{Clock, Hash, SnapshotId, ViewId};

use super::layer::{FileRef, Layer};
use crate::analyse::Record;
use crate::query::Plan;

#[derive(Default)]
pub struct ProjectState {
    pub snapshot: Option<SnapshotId>,
    pub main: Layer,
    pub clock: Clock,
    pub overlays: HashMap<ViewId, Overlay>,
    pub touches: HashMap<ViewId, u64>,
}

#[derive(Default)]
pub struct Overlay {
    pub layer: Layer,
    pub masked: BTreeSet<String>,
    pub clock: Clock,
}

#[derive(Clone)]
pub struct Visible {
    pub path: String,
    pub hash: Hash,
    pub size: u64,
    pub record: Arc<Record>,
    pub edited: bool,
}

impl Visible {
    fn new(path: &str, file: &FileRef, edited: bool) -> Self {
        Self { path: path.to_string(), hash: file.hash.clone(), size: file.size, record: file.record.clone(), edited }
    }
}

impl ProjectState {
    pub fn clock(&self, view: Option<&ViewId>) -> Clock {
        let overlay = self.overlay(view).map(|o| o.clock).unwrap_or_default();
        self.clock.max(overlay)
    }

    pub fn file(&self, view: Option<&ViewId>, path: &str) -> Option<Visible> {
        if let Some(overlay) = self.overlay(view)
            && overlay.masked.contains(path)
        {
            return overlay.layer.get(path).map(|f| Visible::new(path, f, true));
        }
        self.main.get(path).map(|f| Visible::new(path, f, false))
    }

    pub fn each(&self, view: Option<&ViewId>, mut visit: impl FnMut(&str, &FileRef)) {
        let overlay = self.overlay(view);
        for (path, file) in self.main.files() {
            if !overlay.is_some_and(|o| o.masked.contains(path)) {
                visit(path, file);
            }
        }
        for (path, file) in overlay.into_iter().flat_map(|o| o.layer.files()) {
            visit(path, file);
        }
    }

    pub fn candidates(&self, view: Option<&ViewId>, plan: &Plan) -> Vec<Visible> {
        let overlay = self.overlay(view);
        let mut found: Vec<Visible> = self
            .main
            .matching(plan)
            .into_iter()
            .filter(|(path, _)| !overlay.is_some_and(|o| o.masked.contains(*path)))
            .map(|(path, file)| Visible::new(path, file, false))
            .collect();
        if let Some(overlay) = overlay {
            found.extend(overlay.layer.matching(plan).into_iter().map(|(path, file)| Visible::new(path, file, true)));
        }
        found.sort_by(|a, b| a.path.cmp(&b.path));
        found
    }

    fn overlay(&self, view: Option<&ViewId>) -> Option<&Overlay> {
        view.and_then(|v| self.overlays.get(v))
    }
}
