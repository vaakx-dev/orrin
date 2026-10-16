use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use orrin_core::api::{Done, Event, SyncPoke};
use orrin_core::model::{Keep, Project};
use orrin_core::{PcId, ProjectId, SnapshotId};
use tokio::sync::broadcast::error::RecvError;

use crate::book::Peer;
use crate::service::Inner;

#[derive(Default)]
pub struct Pokes {
    held: Mutex<HashMap<PcId, HashSet<ProjectId>>>,
    chasing: Mutex<HashMap<ProjectId, bool>>,
}

impl Pokes {
    pub fn saw(&self, pc: &PcId, projects: &[Project]) {
        let ids = projects.iter().map(|p| p.id.clone()).collect();
        self.held.lock().unwrap_or_else(|e| e.into_inner()).insert(pc.clone(), ids);
    }

    fn holds(&self, pc: &PcId, project: &ProjectId) -> bool {
        self.held.lock().unwrap_or_else(|e| e.into_inner()).get(pc).is_some_and(|ids| ids.contains(project))
    }

    fn begin(&self, project: &ProjectId) -> bool {
        let mut chasing = self.chasing.lock().unwrap_or_else(|e| e.into_inner());
        match chasing.get_mut(project) {
            Some(again) => {
                *again = true;
                false
            }
            None => {
                chasing.insert(project.clone(), false);
                true
            }
        }
    }

    fn finish(&self, project: &ProjectId) -> bool {
        let mut chasing = self.chasing.lock().unwrap_or_else(|e| e.into_inner());
        if chasing.get(project) == Some(&true) {
            chasing.insert(project.clone(), false);
            return false;
        }
        chasing.remove(project);
        true
    }
}

pub fn spawn(inner: Arc<Inner>) {
    let mut events = inner.events.subscribe();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(envelope) => {
                    if let Event::MainMoved { project, main, .. } = envelope.event {
                        poke_followers(&inner, project, main).await;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub fn poked(inner: &Arc<Inner>, input: SyncPoke) -> Done {
    if !inner.pokes.begin(&input.project) {
        return Done {};
    }
    let inner = inner.clone();
    tokio::spawn(async move {
        loop {
            if !inner.has_main(&input.project, &input.main).await
                && let Err(err) = inner.refresh(&input.pc, &input.project).await
            {
                tracing::debug!(%err, project = %input.project, pc = %input.pc, "poke not followed");
            }
            if inner.pokes.finish(&input.project) {
                break;
            }
        }
    });
    Done {}
}

async fn poke_followers(inner: &Arc<Inner>, id: ProjectId, main: SnapshotId) {
    let Ok(project) = inner.hub.projects.get(&id).await else { return };
    let poke = SyncPoke { pc: inner.me.pc.clone(), project: id, main };
    for peer in inner.peers().into_iter().filter(|p| inner.follows(p, &project)) {
        let (inner, poke) = (inner.clone(), poke.clone());
        tokio::spawn(async move {
            if let Err(err) = inner.ask(&peer.pc, poke.clone()).await {
                tracing::debug!(%err, pc = %peer.pc, project = %poke.project, "peer not poked");
            }
        });
    }
}

impl Inner {
    fn follows(&self, peer: &Peer, project: &Project) -> bool {
        let wants = match peer.settings.keep {
            Keep::None => false,
            Keep::Active => self.pokes.holds(&peer.pc, &project.id),
            Keep::All => true,
        };
        peer.online && peer.pc != project.home_pc && wants
    }

    async fn has_main(&self, project: &ProjectId, main: &SnapshotId) -> bool {
        self.hub.projects.get(project).await.is_ok_and(|p| p.main == *main)
    }
}
