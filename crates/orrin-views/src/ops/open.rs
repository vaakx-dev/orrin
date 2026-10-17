use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use orrin_core::api::{ProjectOpened, ProjectsImport, ViewCreate};
use orrin_core::model::{Entry, Project, Snapshot, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Store};
use orrin_core::{Error, ProjectId, Result, ViewId, now_ms};
use rayon::prelude::*;

use crate::disk::rules::Rules;
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::pools;
use crate::slot::{Slot, ViewRecord};
use crate::watch::watch;

const USER: &str = "user";

impl Inner {
    pub async fn import(&self, input: ProjectsImport) -> Result<ProjectOpened> {
        let root = folder(&input.path)?;
        self.ensure_free(&root).await?;
        let name = input.name.unwrap_or_else(|| folder_name(&root));
        let mut stats = Stats::default();
        let rules = Rules::load(&root, &[]);
        let snapshot = pools::files().install(|| snapshot_folder(&*self.hub.store, &rules, &mut stats))?;
        let main = self.hub.store.put_snapshot(&snapshot)?;
        let project = Project {
            id: ProjectId::new(),
            name,
            home_pc: self.me.pc.clone(),
            main,
            settings: Default::default(),
            created_at: now_ms(),
        };
        self.hub.projects.save(project.clone()).await?;
        let view = self.open_view(user_view(&project, root), stats, true).await?;
        let paths: Vec<String> = snapshot.entries.into_iter().map(|e| e.path).collect();
        let files = paths.len() as u64;
        let (hub, id) = (self.hub.clone(), project.id.clone());
        tokio::spawn(async move {
            if let Err(err) = hub.index.touched(&id, None, &paths).await {
                tracing::warn!("index.touched failed for imported project {id}: {err}");
            }
        });
        Ok(ProjectOpened { project, view: Some(view), files })
    }

    pub async fn adopt(&self, project: &ProjectId, path: &Path) -> Result<View> {
        let project = self.hub.projects.get(project).await?;
        let root = folder(path)?;
        self.ensure_free(&root).await?;
        self.open_view(user_view(&project, root), Stats::default(), false).await
    }

    pub async fn create(&self, input: ViewCreate) -> Result<View> {
        let project = self.hub.projects.get(&input.project).await?;
        let base = input.base.unwrap_or(project.main);
        let folder = self.folder_at(&project.id, &base).await?;
        let view = View {
            id: folder.id,
            project: project.id,
            owner: input.owner,
            kind: ViewKind::Worker,
            path: folder.root,
            base,
            claims: input.claims,
            state: ViewState::Open,
            created_at: now_ms(),
            landed: None,
        };
        let view = self.open_view(view, folder.stats, true).await?;
        self.link_latest(&view.id).await;
        Ok(view)
    }

    async fn open_view(&self, view: View, stats: Stats, clean: bool) -> Result<View> {
        let record =
            ViewRecord { view: view.clone(), captured: Edits::new(), closed_at: None, linked: BTreeMap::new() };
        self.save(&record)?;
        let slot = Slot::new(record, stats);
        self.save_stats(&slot)?;
        if clean && let Err(err) = self.hub.index.touched(&view.project, Some(&view.id), &[]).await {
            tracing::warn!("index could not start view {}: {err}", view.id);
        }
        watch(self, &slot);
        self.insert(slot);
        Ok(view)
    }

    pub fn materialise_all(&self, snapshot: &Snapshot, root: &Path) -> Result<Stats> {
        let dirs: BTreeSet<PathBuf> =
            snapshot.entries.iter().filter_map(|e| root.join(&e.path).parent().map(Path::to_path_buf)).collect();
        std::fs::create_dir_all(root)?;
        for dir in &dirs {
            std::fs::create_dir_all(dir)?;
        }
        let written: Vec<_> =
            snapshot.entries.par_iter().map(|entry| self.put_entry(ViewKind::Worker, root, entry)).collect();
        let mut stats = Stats::default();
        for (entry, meta) in snapshot.entries.iter().zip(written) {
            stats.remember(&entry.path, &meta?, entry.hash.clone());
        }
        Ok(stats)
    }

    async fn ensure_free(&self, root: &Path) -> Result<()> {
        for slot in self.slots() {
            if slot.root == root && Slot::is_open(&*slot.record.lock().await) {
                return Err(Error::conflict(format!("{} is already view {}", root.display(), slot.id)));
            }
        }
        Ok(())
    }
}

fn user_view(project: &Project, root: PathBuf) -> View {
    View {
        id: ViewId::new(),
        project: project.id.clone(),
        owner: USER.to_string(),
        kind: ViewKind::User,
        path: root,
        base: project.main.clone(),
        claims: Vec::new(),
        state: ViewState::Open,
        created_at: now_ms(),
        landed: None,
    }
}

fn folder(path: &Path) -> Result<PathBuf> {
    let root = std::path::absolute(path)?;
    if !root.is_dir() {
        return Err(Error::invalid(format!("not a folder: {}", root.display())));
    }
    Ok(root)
}

fn folder_name(root: &Path) -> String {
    root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".to_string())
}

fn snapshot_folder(store: &dyn Store, rules: &Rules, stats: &mut Stats) -> Result<Snapshot> {
    let mut found = rules.walk();
    found.sort_by_key(|f| Reverse(f.meta.len()));
    let hashes: Vec<Result<_>> = found.par_iter().map(|f| store.put_file(&f.abs)).collect();
    let entries = found
        .into_iter()
        .zip(hashes)
        .map(|(found, hash)| {
            let state = stats.remember(&found.rel, &found.meta, hash?);
            Ok(Entry { path: found.rel, hash: state.hash, size: state.size, mode: state.mode })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Snapshot::new(entries))
}
