use orrin_core::api::{Event, PcsListOutput};
use orrin_core::model::Pc;
use orrin_core::{Error, PcId, Result, now_ms};

use crate::book::Peer;
use crate::service::Inner;

impl Inner {
    pub fn announce_seen(&self, pc: Pc) {
        self.events.emit(Event::PcSeen { pc });
    }

    pub fn saw(&self, pc: &PcId, update: impl FnOnce(&mut Peer)) -> Result<Peer> {
        let (peer, changed) = self
            .edit_book(|book| {
                let peer = book.get_mut(pc)?;
                let before = peer.clone();
                update(peer);
                peer.online = true;
                peer.last_seen = Some(now_ms());
                let changed = before.online != peer.online
                    || before.name != peer.name
                    || before.address != peer.address
                    || before.settings != peer.settings;
                Some((peer.clone(), changed))
            })?
            .ok_or_else(|| Error::unauthorized(format!("{pc} is not paired with this PC")))?;
        if changed {
            self.announce_seen(peer.pc());
        }
        Ok(peer)
    }

    pub fn lost(&self, pc: &PcId) {
        let gone = self.read_book(|book| {
            let peer = book.get_mut(pc).filter(|p| p.online)?;
            peer.online = false;
            Some(peer.pc())
        });
        if let Some(pc) = gone {
            self.announce_seen(pc);
        }
    }

    pub fn pcs(&self) -> PcsListOutput {
        let me = Pc {
            id: self.me.pc.clone(),
            name: self.me.name.clone(),
            address: Some(self.address()),
            me: true,
            online: true,
            last_seen: Some(now_ms()),
            settings: self.my_settings(),
        };
        let mut pcs = vec![me];
        pcs.extend(self.peers().iter().map(Peer::pc));
        PcsListOutput { pcs }
    }
}
