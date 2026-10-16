use std::collections::{BTreeSet, HashSet};

use orrin_core::api::{Event, SyncChanges, ViewList};
use orrin_core::model::{Change, Keep, Project, ViewKind, ViewState};
use orrin_core::{Error, Hash, PcId, ProjectId, Result};

use crate::book::Peer;
use crate::changes::history;
use crate::objects::delta;
use crate::service::Inner;
use crate::space::Space;

impl Inner {
    pub async fn refresh(&self, pc: &PcId, project: &ProjectId) -> Result<()> {
        let peer = self.peer(pc)?;
        let theirs = self.ask(pc, self.my_announce().await).await?;
        let theirs = theirs.projects.into_iter().find(|p| p.id == *project);
        let theirs = theirs.ok_or_else(|| Error::not_found(format!("{pc} does not announce {project}")))?;
        let settings = self.my_settings();
        self.follow(&peer, theirs, settings.keep, &mut Space::new(&settings)).await
    }

    pub async fn follow(&self, peer: &Peer, theirs: Project, keep: Keep, space: &mut Space) -> Result<()> {
        let _one_at_a_time = self.following.lock().await;
        if theirs.home_pc == self.me.pc || keep == Keep::None {
            return Ok(());
        }
        let local = self.hub.projects.get(&theirs.id).await.ok();
        if local.as_ref() == Some(&theirs) || (local.is_none() && keep == Keep::Active) {
            return Ok(());
        }
        let authoritative = peer.pc == theirs.home_pc;
        if local.as_ref().is_some_and(|l| l.main == theirs.main) {
            return if authoritative { self.hub.projects.save(theirs).await } else { Ok(()) };
        }
        let known = match &local {
            Some(_) => history(&self.hub, &theirs.id).await?,
            None => Vec::new(),
        };
        let after = known.last().map(|c| c.id.clone());
        let reply = self.ask(&peer.pc, SyncChanges { project: theirs.id.clone(), after }).await?;
        let known: HashSet<_> = known.into_iter().map(|c| c.id).collect();
        let fresh: Vec<Change> = reply.changes.into_iter().filter(|c| !known.contains(&c.id)).collect();
        let theirs = reply.project;
        if !authoritative && !fast_forward(local.as_ref(), &fresh) {
            return Ok(());
        }
        space.check(&self.home.store())?;
        self.pull(&peer.pc, vec![theirs.main.clone()]).await?;
        let snapshot = self.hub.store.snapshot(&theirs.main)?;
        let wanted = delta(self.hub.store.as_ref(), local.as_ref().map(|l| &l.main), &snapshot);
        self.pull(&peer.pc, wanted).await?;
        if keep == Keep::All
            && let Err(err) = self.pull(&peer.pc, history_objects(&fresh)).await
        {
            tracing::debug!(%err, project = %theirs.id, "some history objects not fetched");
        }
        self.adopt(local, theirs, fresh).await
    }

    async fn adopt(&self, local: Option<Project>, theirs: Project, fresh: Vec<Change>) -> Result<()> {
        let moved = local.as_ref().is_some_and(|l| l.main != theirs.main);
        let paths: BTreeSet<String> = fresh.iter().flat_map(|c| c.files.keys().cloned()).collect();
        let last = fresh.last().map(|c| c.id.clone());
        self.hub.projects.save(theirs.clone()).await?;
        if !fresh.is_empty() {
            self.hub.projects.append(fresh).await?;
        }
        if let Some(change) = last.filter(|_| moved) {
            self.events.emit(Event::MainMoved { project: theirs.id.clone(), main: theirs.main.clone(), change });
        }
        let paths: Vec<String> = paths.into_iter().collect();
        if let Err(err) = self.hub.index.touched(&theirs.id, None, &paths).await {
            tracing::debug!(%err, "index not updated after following main");
        }
        self.advance_user_views(&theirs).await;
        Ok(())
    }

    async fn advance_user_views(&self, project: &Project) {
        let Ok(list) = self.hub.views.list(ViewList { project: Some(project.id.clone()) }).await else { return };
        let open = list.views.into_iter().filter(|v| v.kind == ViewKind::User && v.state == ViewState::Open);
        for view in open {
            if let Err(err) = self.hub.views.advance(&view.id, &project.main).await {
                tracing::warn!(%err, view = %view.id, "user's folder not updated");
            }
        }
    }
}

fn fast_forward(local: Option<&Project>, fresh: &[Change]) -> bool {
    match local {
        None => true,
        Some(local) => fresh.first().is_some_and(|c| c.parent == local.main),
    }
}

fn history_objects(changes: &[Change]) -> Vec<Hash> {
    changes
        .iter()
        .flat_map(|c| {
            let files = c.files.values().flatten().map(|f| f.hash.clone());
            [c.parent.clone(), c.snapshot.clone()].into_iter().chain(files)
        })
        .collect()
}
