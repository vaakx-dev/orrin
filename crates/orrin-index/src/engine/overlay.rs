use std::collections::BTreeSet;

use orrin_core::model::clean_path;
use orrin_core::{Clock, ProjectId, Result, ViewId};

use super::content::Item;
use super::state::Overlay;
use super::{Engine, ProjectIndex};

impl Engine {
    pub(super) async fn start_clean(&self, project: &ProjectId, view: &ViewId) -> Result<Clock> {
        let clock = self.tick()?;
        let index = self.index(project);
        index.write().overlays.entry(view.clone()).or_insert_with(|| Overlay { clock, ..Overlay::default() });
        Ok(clock)
    }

    pub(super) async fn ensure_overlay(&self, project: &ProjectId, index: &ProjectIndex, view: &ViewId) -> Result<()> {
        while !index.has_overlay(view) {
            let generation = index.read().touches.get(view).copied().unwrap_or(0);
            let overlay = self.capture_overlay(project, view).await?;
            let _gate = index.gate.lock().await;
            let mut state = index.write();
            let current = state.touches.get(view).copied().unwrap_or(0);
            if current == generation && !state.overlays.contains_key(view) {
                state.overlays.insert(view.clone(), overlay);
            }
        }
        Ok(())
    }

    pub(super) async fn touch_view(
        &self,
        project: &ProjectId,
        index: &ProjectIndex,
        view: &ViewId,
        paths: &[String],
    ) -> Result<Clock> {
        let _gate = index.gate.lock().await;
        if !index.has_overlay(view) {
            let clock = self.tick()?;
            *index.write().touches.entry(view.clone()).or_default() += 1;
            return Ok(clock);
        }
        let mut items = Vec::new();
        let mut gone = Vec::new();
        for path in paths {
            let Ok(path) = clean_path(path) else { continue };
            match self.read_view(project, view, &path).await {
                Ok(Some(file)) => items.push(Item { path, hash: file.hash, size: file.size, bytes: file.bytes }),
                Ok(None) => gone.push(path),
                Err(err) => tracing::warn!("index skipped {path} in {view}: {err}"),
            }
        }
        let resolved = self.resolve(items, Some((project, view))).await?;
        let clock = self.tick()?;
        let mut state = index.write();
        let overlay = state.overlays.entry(view.clone()).or_default();
        for path in gone {
            overlay.layer.remove(&path);
            overlay.masked.insert(path);
        }
        for (item, record) in resolved {
            overlay.masked.insert(item.path.clone());
            overlay.layer.insert(item.path, item.hash, item.size, record);
        }
        overlay.clock = clock;
        Ok(clock)
    }

    async fn capture_overlay(&self, project: &ProjectId, view: &ViewId) -> Result<Overlay> {
        let edits = self.hub.views.capture(view).await?;
        let masked: BTreeSet<String> = edits.keys().cloned().collect();
        let items = edits
            .into_iter()
            .filter_map(|(path, state)| state.map(|s| Item { path, hash: s.hash, size: s.size, bytes: None }))
            .collect();
        let resolved = self.resolve(items, Some((project, view))).await?;
        let mut overlay = Overlay { masked, clock: self.tick()?, ..Overlay::default() };
        for (item, record) in resolved {
            overlay.layer.insert(item.path, item.hash, item.size, record);
        }
        Ok(overlay)
    }
}
