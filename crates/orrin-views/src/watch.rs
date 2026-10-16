use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use orrin_core::ViewId;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::disk::rules::{Rules, rel_path};
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::slot::Slot;

const SETTLE: Duration = Duration::from_millis(200);

pub struct Hit {
    view: ViewId,
    path: PathBuf,
}

pub fn watch(inner: &Inner, slot: &Slot) {
    let tx = inner.hits.clone();
    let view = slot.id.clone();
    let handler = move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.kind.is_access() {
            return;
        }
        for path in event.paths {
            let _ = tx.send(Hit { view: view.clone(), path });
        }
    };
    let watcher =
        notify::recommended_watcher(handler).and_then(|mut w| w.watch(&slot.root, RecursiveMode::Recursive).map(|_| w));
    match watcher {
        Ok(watcher) => slot.set_watcher(Some(watcher)),
        Err(err) => tracing::warn!("cannot watch view {} at {}: {err}", slot.id, slot.root.display()),
    }
}

pub async fn run(inner: Arc<Inner>, mut rx: UnboundedReceiver<Hit>) {
    while let Some(first) = rx.recv().await {
        tokio::time::sleep(SETTLE).await;
        let mut by_view: BTreeMap<ViewId, BTreeSet<PathBuf>> = BTreeMap::new();
        by_view.entry(first.view).or_default().insert(first.path);
        while let Ok(hit) = rx.try_recv() {
            by_view.entry(hit.view).or_default().insert(hit.path);
        }
        for (view, paths) in by_view {
            outside_edits(&inner, &view, paths).await;
        }
    }
}

async fn outside_edits(inner: &Inner, id: &ViewId, paths: BTreeSet<PathBuf>) {
    let Ok(slot) = inner.slot(id) else { return };
    let rules = slot.rules();
    let changed: BTreeSet<String> = {
        let mut stats = slot.stats();
        paths.iter().flat_map(|abs| classify(&slot.root, &rules, &mut stats, abs)).collect()
    };
    if !changed.is_empty() {
        inner.touched(&slot, changed.into_iter().collect()).await;
    }
}

fn classify(root: &Path, rules: &Rules, stats: &mut Stats, abs: &Path) -> Vec<String> {
    let Some(rel) = rel_path(root, abs) else { return Vec::new() };
    match std::fs::symlink_metadata(abs) {
        Ok(meta) if meta.is_dir() => Vec::new(),
        Ok(meta) if rules.ignores(&rel, false) || stats.unchanged(&rel, &meta) => Vec::new(),
        Ok(_) => vec![rel],
        Err(_) if rules.ignores(&rel, false) => Vec::new(),
        Err(_) => stats.forget_under(&rel),
    }
}
