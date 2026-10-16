use std::sync::Arc;
use std::time::Duration;

use orrin_core::model::Keep;
use tokio::time::MissedTickBehavior;

use crate::announce::learn;
use crate::book::Peer;
use crate::service::Inner;
use crate::space::Space;

const EVERY: Duration = Duration::from_secs(10);

pub fn spawn(inner: Arc<Inner>) {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            inner.round().await;
        }
    });
}

impl Inner {
    async fn round(&self) {
        let settings = self.my_settings();
        let mut space = Space::new(&settings);
        for peer in self.peers() {
            self.visit(peer, settings.keep, &mut space).await;
        }
    }

    async fn visit(&self, peer: Peer, keep: Keep, space: &mut Space) {
        let mine = self.my_announce().await;
        let theirs = match self.ask(&peer.pc, mine.clone()).await {
            Ok(theirs) => theirs,
            Err(err) => {
                tracing::debug!(%err, pc = %peer.pc, "peer not reached");
                self.lost(&peer.pc);
                return;
            }
        };
        let peer = match self.saw(&peer.pc, |p| learn(p, &theirs)) {
            Ok(peer) => peer,
            Err(err) => return tracing::warn!(%err, pc = %peer.pc, "peer book not saved"),
        };
        self.pokes.saw(&peer.pc, &theirs.projects);
        for project in theirs.projects.iter().cloned() {
            let id = project.id.clone();
            if let Err(err) = self.follow(&peer, project, keep, space).await {
                tracing::warn!(%err, project = %id, pc = %peer.pc, "project not followed");
            }
        }
        for project in mine.projects.iter().filter(|p| p.home_pc == self.me.pc) {
            if let Err(err) = self.offer(&peer, project, &theirs.projects).await {
                tracing::warn!(%err, project = %project.id, pc = %peer.pc, "objects not pushed");
            }
        }
    }
}
