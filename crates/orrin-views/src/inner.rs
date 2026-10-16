use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use orrin_core::api::Event;
use orrin_core::config::ViewsConfig;
use orrin_core::service::{Hub, Setup};
use orrin_core::{Clock, Error, Events, Home, Identity, ProjectId, Result, ViewId};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::disk::files::write_fresh;
use crate::disk::rules::Rules;
use crate::disk::stats::Stats;
use crate::slot::{Slot, ViewRecord};
use crate::watch::Hit;

pub struct Inner {
    pub hub: Hub,
    pub home: Home,
    pub config: ViewsConfig,
    pub me: Identity,
    pub events: Events,
    pub hits: UnboundedSender<Hit>,
    pub hits_rx: Mutex<Option<UnboundedReceiver<Hit>>>,
    slots: RwLock<HashMap<ViewId, Arc<Slot>>>,
}

impl Inner {
    pub fn new(setup: &Setup) -> Result<Self> {
        let (hits, rx) = unbounded_channel();
        let inner = Self {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.views.clone(),
            me: setup.me.clone(),
            events: setup.events.clone(),
            hits,
            hits_rx: Mutex::new(Some(rx)),
            slots: RwLock::default(),
        };
        inner.load()?;
        Ok(inner)
    }

    fn load(&self) -> Result<()> {
        for entry in std::fs::read_dir(self.home.dir("views")?)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let record: ViewRecord = serde_json::from_slice(&std::fs::read(&path)?)?;
                self.insert(Slot::new(record, Stats::default()));
            }
        }
        Ok(())
    }

    pub fn worker_root(&self, id: &ViewId) -> PathBuf {
        self.home.views().join(id.as_str())
    }

    fn meta_path(&self, id: &ViewId) -> PathBuf {
        self.home.views().join(format!("{id}.json"))
    }

    pub fn save(&self, record: &ViewRecord) -> Result<()> {
        write_fresh(&self.meta_path(&record.view.id), &serde_json::to_vec_pretty(record)?)
    }

    pub fn forget(&self, id: &ViewId) -> Result<()> {
        self.slots.write().unwrap_or_else(|e| e.into_inner()).remove(id);
        crate::disk::files::remove_if_present(&self.meta_path(id))
    }

    pub fn insert(&self, slot: Arc<Slot>) {
        self.slots.write().unwrap_or_else(|e| e.into_inner()).insert(slot.id.clone(), slot);
    }

    pub fn slots(&self) -> Vec<Arc<Slot>> {
        self.slots.read().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }

    pub fn slot(&self, id: &ViewId) -> Result<Arc<Slot>> {
        let slots = self.slots.read().unwrap_or_else(|e| e.into_inner());
        slots.get(id).cloned().ok_or_else(|| Error::not_found(format!("no view {id}")))
    }

    pub fn slot_in(&self, project: &ProjectId, id: &ViewId) -> Result<Arc<Slot>> {
        let slot = self.slot(id)?;
        if &slot.project != project {
            return Err(Error::not_found(format!("view {id} is not in project {project}")));
        }
        Ok(slot)
    }

    pub async fn open_slot(&self, project: &ProjectId, id: &ViewId) -> Result<Arc<Slot>> {
        let slot = self.slot_in(project, id)?;
        if !Slot::is_open(&*slot.record.lock().await) {
            return Err(Error::conflict(format!("view {id} is closed")));
        }
        Ok(slot)
    }

    pub async fn refresh_rules(&self, slot: &Slot) -> Arc<Rules> {
        let settings = self.hub.projects.get(&slot.project).await.map(|p| p.settings.ignore).unwrap_or_default();
        slot.set_rules(Rules::load(&slot.root, &settings))
    }

    pub async fn touched(&self, slot: &Slot, paths: Vec<String>) -> Clock {
        let clock = match self.hub.index.touched(&slot.project, Some(&slot.id), &paths).await {
            Ok(clock) => clock,
            Err(err) => {
                tracing::warn!("index.touched failed for view {}: {err}", slot.id);
                self.hub.index.clock(&slot.project, Some(&slot.id)).await
            }
        };
        self.events.emit(Event::ViewChanged { view: slot.id.clone(), paths, clock });
        clock
    }
}
