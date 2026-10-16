use orrin_core::Result;
use orrin_core::api::SyncAnnounce;

use crate::book::Peer;
use crate::service::Inner;

impl Inner {
    pub async fn my_announce(&self) -> SyncAnnounce {
        let projects = match self.hub.projects.list().await {
            Ok(list) => list.projects,
            Err(err) => {
                tracing::debug!(%err, "no projects to announce");
                Vec::new()
            }
        };
        SyncAnnounce {
            pc: self.me.pc.clone(),
            name: self.me.name.clone(),
            address: Some(self.address()),
            projects,
            settings: self.my_settings(),
        }
    }

    pub async fn announce(&self, input: SyncAnnounce) -> Result<SyncAnnounce> {
        self.saw(&input.pc, |peer| learn(peer, &input))?;
        self.pokes.saw(&input.pc, &input.projects);
        Ok(self.my_announce().await)
    }
}

pub fn learn(peer: &mut Peer, announce: &SyncAnnounce) {
    peer.name = announce.name.clone();
    peer.settings = announce.settings.clone();
    if let Some(address) = announce.address.as_ref().filter(|a| !a.is_empty()) {
        peer.address = address.clone();
    }
}
