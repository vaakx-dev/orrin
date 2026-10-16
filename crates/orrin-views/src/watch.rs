use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use orrin_core::model::ViewKind;
use orrin_core::{Hash, ViewId};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::disk::rules::{Rules, rel_path};
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::slot::Slot;

const SETTLE: Duration = Duration::from_millis(200);

pub struct Hit {
    view: ViewId,
    path: PathBuf,
    modified: bool,
}

enum Seen {
    Changed(Vec<String>),
    Same(String, Hash),
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
            let _ = tx.send(Hit { view: view.clone(), path, modified: event.kind.is_modify() });
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
        let mut by_view: BTreeMap<ViewId, BTreeMap<PathBuf, bool>> = BTreeMap::new();
        let mut add = |hit: Hit| *by_view.entry(hit.view).or_default().entry(hit.path).or_default() |= hit.modified;
        add(first);
        while let Ok(hit) = rx.try_recv() {
            add(hit);
        }
        for (view, paths) in by_view {
            outside_edits(&inner, &view, paths).await;
        }
    }
}

async fn outside_edits(inner: &Inner, id: &ViewId, paths: BTreeMap<PathBuf, bool>) {
    let Ok(slot) = inner.slot(id) else { return };
    let rules = slot.rules();
    let mut changed = BTreeSet::new();
    let mut poked = Vec::new();
    {
        let mut stats = slot.stats();
        for (abs, modified) in &paths {
            match classify(&slot.root, &rules, &mut stats, abs) {
                Some(Seen::Changed(rels)) => changed.extend(rels),
                Some(Seen::Same(rel, hash)) if *modified => poked.push((rel, hash)),
                _ => {}
            }
        }
    }
    if slot.kind == ViewKind::Worker && !poked.is_empty() {
        let _record = slot.record.lock().await;
        for (rel, hash) in poked {
            if let Err(err) = inner.own(&slot, &rel, hash) {
                tracing::warn!("cannot give {rel} in view {} its own copy: {err}", slot.id);
            }
        }
    }
    if !changed.is_empty() {
        inner.touched(&slot, changed.into_iter().collect()).await;
    }
}

fn classify(root: &Path, rules: &Rules, stats: &mut Stats, abs: &Path) -> Option<Seen> {
    let rel = rel_path(root, abs)?;
    match std::fs::symlink_metadata(abs) {
        Ok(meta) if meta.is_dir() || rules.ignores(&rel, false) => None,
        Ok(meta) => match stats.known(&rel, &meta) {
            Some(hash) => Some(Seen::Same(rel, hash)),
            None => Some(Seen::Changed(vec![rel])),
        },
        Err(_) if rules.ignores(&rel, false) => None,
        Err(_) => Some(Seen::Changed(stats.forget_under(&rel))),
    }
}
