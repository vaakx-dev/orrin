use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orrin_core::api::{PackageFound, SyncPackage, ViewList};
use orrin_core::model::{PackageBuild, Rule, ViewKind, ViewState};
use orrin_core::service::{Hub, PackageNeed, PackageReady, Packages, Service, Setup, Store};
use orrin_core::{Error, Hash, ProjectId, Result, SnapshotId};
use serde::Serialize;

use crate::area::{Area, Kind, remove};
use crate::source::Source;
use crate::{build, folder, peers};

pub struct PackageService {
    hub: Hub,
    area: Arc<Area>,
    keep_days: u32,
    locks: Mutex<HashMap<Hash, Arc<tokio::sync::Mutex<()>>>>,
}

struct Located {
    source: Source,
    source_key: Hash,
    snapshot: Option<SnapshotId>,
    key: Hash,
}

#[derive(Serialize)]
struct BuildKey<'a> {
    os: &'static str,
    arch: &'static str,
    source: &'a Hash,
    build: &'a Option<PackageBuild>,
    rules: &'a BTreeMap<String, Rule>,
}

impl PackageService {
    pub fn new(setup: &Setup) -> Result<Self> {
        Ok(Self {
            hub: setup.hub.clone(),
            area: Arc::new(Area::open(setup.home.packages())?),
            keep_days: setup.config.packages.keep_days,
            locks: Mutex::default(),
        })
    }

    async fn locate(&self, need: &PackageNeed) -> Result<Located> {
        let base = self.base(&need.project).await;
        let source = Source::of(&need.name, &need.package, base.as_deref())?;
        let (source_key, snapshot) = match &source {
            Source::Folder { dir } => {
                let snapshot = snapshot_of(self.hub.store.clone(), dir.clone()).await?;
                (snapshot.clone(), Some(snapshot))
            }
            _ => {
                let key = source.key();
                let known = self.area.get(&*self.hub.store, Kind::Source, &key);
                (key, known)
            }
        };
        let parts = BuildKey {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            source: &source_key,
            build: &need.package.build,
            rules: &need.rules,
        };
        let key = Hash::of(&serde_json::to_vec(&parts)?);
        Ok(Located { source, source_key, snapshot, key })
    }

    async fn base(&self, project: &ProjectId) -> Option<PathBuf> {
        let views = self.hub.views.list(ViewList { project: Some(project.clone()) }).await.ok()?.views;
        let mine = views.into_iter().find(|v| v.kind == ViewKind::User && v.state == ViewState::Open);
        mine.map(|view| view.path)
    }

    async fn source(&self, name: &str, located: &Located) -> Result<SnapshotId> {
        if let Some(snapshot) = &located.snapshot {
            return Ok(snapshot.clone());
        }
        let into = self.area.fetch_dir(&located.source_key);
        let snapshot = match located.source.fetch(&into).await {
            Ok(root) => snapshot_of(self.hub.store.clone(), root).await,
            Err(err) => Err(err),
        };
        remove(&into);
        let snapshot = snapshot?;
        self.area.put(Kind::Source, &located.source_key, name, &snapshot)?;
        Ok(snapshot)
    }

    async fn ready(&self, key: &Hash, name: &str, snapshot: &SnapshotId, note: String) -> Result<PackageReady> {
        self.area.put(Kind::Built, key, name, snapshot)?;
        let (area, store, at, id) = (self.area.clone(), self.hub.store.clone(), key.clone(), snapshot.clone());
        let dir = blocking(move || area.place(&*store, &at, &id)).await?;
        Ok(PackageReady { key: key.clone(), snapshot: snapshot.clone(), dir, note })
    }

    fn lock(&self, key: &Hash) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        locks.entry(key.clone()).or_default().clone()
    }
}

impl Service for PackageService {}

#[async_trait]
impl Packages for PackageService {
    async fn key(&self, need: PackageNeed) -> Result<Hash> {
        Ok(self.locate(&need).await?.key)
    }

    async fn ensure(&self, need: PackageNeed) -> Result<PackageReady> {
        let located = self.locate(&need).await?;
        let (key, name) = (&located.key, need.name.as_str());
        let lock = self.lock(key);
        let _held = lock.lock().await;
        if let Some(snapshot) = self.area.get(&*self.hub.store, Kind::Built, key) {
            return self.ready(key, name, &snapshot, "ready".into()).await;
        }
        if let Some((pc, snapshot)) = peers::fetch(&self.hub, key).await {
            return self.ready(key, name, &snapshot, format!("copied from {pc}")).await;
        }
        let source = self.source(name, &located).await?;
        let Some(how) = &need.package.build else {
            return self.ready(key, name, &source, "fetched".into()).await;
        };
        let built = build::run(&self.hub, &need, &source, how).await?;
        let note = match built.job.cached {
            true => "built (cached)".to_string(),
            false => format!("built in job {}", built.job.id),
        };
        self.ready(key, name, &built.snapshot, note).await
    }

    async fn find(&self, input: SyncPackage) -> Result<PackageFound> {
        Ok(PackageFound { snapshot: self.area.get(&*self.hub.store, Kind::Built, &input.key) })
    }

    async fn clean(&self) -> Result<u32> {
        let (area, keep) = (self.area.clone(), self.keep_days);
        blocking(move || Ok(area.clean(keep))).await
    }

    async fn roots(&self) -> Result<Vec<Hash>> {
        Ok(self.area.roots())
    }
}

async fn snapshot_of(store: Arc<dyn Store>, dir: PathBuf) -> Result<SnapshotId> {
    blocking(move || {
        if !dir.is_dir() {
            return Err(Error::not_found(format!("no package folder {}", dir.display())));
        }
        store.put_snapshot(&folder::snapshot(&*store, &dir)?)
    })
    .await
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|err| Error::internal(err.to_string()))?
}
