use std::path::{Path, PathBuf};

use orrin_core::api::{Event, ProjectOpened, ProjectsImport, ViewCreate};
use orrin_core::model::{Entry, Project, Snapshot, View, ViewKind, ViewState};
use orrin_core::service::{Edits, Store};
use orrin_core::{Error, ProjectId, Result, ViewId, now_ms};

use crate::disk::rules::Rules;
use crate::disk::stats::Stats;
use crate::inner::Inner;
use crate::slot::{Slot, ViewRecord};
use crate::watch::watch;

const USER: &str = "user";

impl Inner {
    pub async fn import(&self, input: ProjectsImport) -> Result<ProjectOpened> {
        let root = folder(&input.path)?;
        self.ensure_free(&root).await?;
        let name = input.name.unwrap_or_else(|| folder_name(&root));
        let mut stats = Stats::default();
        let snapshot = snapshot_folder(&*self.hub.store, &Rules::load(&root, &[]), &mut stats)?;
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
        self.events.emit(Event::ProjectCreated { project: project.clone() });
        let view = self.open_view(user_view(&project, root), stats)?;
        let paths: Vec<String> = snapshot.entries.into_iter().map(|e| e.path).collect();
        if let Err(err) = self.hub.index.touched(&project.id, None, &paths).await {
            tracing::warn!("index.touched failed for imported project {}: {err}", project.id);
        }
        Ok(ProjectOpened { project, view: Some(view), files: paths.len() as u64 })
    }

    pub async fn adopt(&self, project: &ProjectId, path: &Path) -> Result<View> {
        let project = self.hub.projects.get(project).await?;
        let root = folder(path)?;
        self.ensure_free(&root).await?;
        self.open_view(user_view(&project, root), Stats::default())
    }

    pub async fn create(&self, input: ViewCreate) -> Result<View> {
        let project = self.hub.projects.get(&input.project).await?;
        let base = input.base.unwrap_or(project.main);
        let snapshot = self.hub.store.snapshot(&base)?;
        let id = ViewId::new();
        let root = self.worker_root(&id);
        let stats = self.materialise_all(&snapshot, &root).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&root);
        })?;
        let view = View {
            id,
            project: project.id,
            owner: input.owner,
            kind: ViewKind::Worker,
            path: root,
            base,
            claims: input.claims,
            state: ViewState::Open,
            created_at: now_ms(),
            landed: None,
        };
        self.open_view(view, stats)
    }

    fn open_view(&self, view: View, stats: Stats) -> Result<View> {
        let record = ViewRecord { view: view.clone(), captured: Edits::new(), closed_at: None };
        self.save(&record)?;
        let slot = Slot::new(record, stats);
        watch(self, &slot);
        self.insert(slot);
        Ok(view)
    }

    fn materialise_all(&self, snapshot: &Snapshot, root: &Path) -> Result<Stats> {
        std::fs::create_dir_all(root)?;
        let mut stats = Stats::default();
        for entry in &snapshot.entries {
            self.place(ViewKind::Worker, root, entry, &mut stats)?;
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
    let entries = rules
        .walk()
        .into_iter()
        .map(|found| {
            let state = stats.remember_put(&found.rel, &found.abs, store)?;
            Ok(Entry { path: found.rel, hash: state.hash, size: state.size, mode: state.mode })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Snapshot::new(entries))
}
