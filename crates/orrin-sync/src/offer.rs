use orrin_core::Result;
use orrin_core::model::{Keep, Project};

use crate::book::Peer;
use crate::objects::delta;
use crate::service::Inner;

impl Inner {
    pub async fn offer(&self, peer: &Peer, project: &Project, theirs: &[Project]) -> Result<()> {
        let copy = theirs.iter().find(|p| p.id == project.id);
        let wanted = match (peer.settings.keep, copy) {
            (Keep::None, _) | (Keep::Active, None) => false,
            (_, Some(copy)) => copy.main != project.main,
            (Keep::All, None) => true,
        };
        if !wanted {
            return Ok(());
        }
        let snapshot = self.hub.store.snapshot(&project.main)?;
        let mut hashes = vec![project.main.clone()];
        hashes.extend(delta(self.hub.store.as_ref(), copy.map(|c| &c.main), &snapshot));
        self.push(&peer.pc, hashes).await
    }
}
