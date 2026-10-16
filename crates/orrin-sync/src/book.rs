use std::path::PathBuf;

use orrin_core::model::{Pc, PcSettings};
use orrin_core::{Error, Home, Millis, PcId, Result};
use serde::{Deserialize, Serialize};

use crate::secret::same;
use crate::service::Inner;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {
    pub pc: PcId,
    pub name: String,
    pub address: String,
    pub send_key: String,
    pub accept_key: String,
    #[serde(default)]
    pub settings: PcSettings,
    #[serde(default)]
    pub last_seen: Option<Millis>,
    #[serde(skip)]
    pub online: bool,
}

impl Peer {
    pub fn pc(&self) -> Pc {
        Pc {
            id: self.pc.clone(),
            name: self.name.clone(),
            address: Some(self.address.clone()),
            me: false,
            online: self.online,
            last_seen: self.last_seen,
            settings: self.settings.clone(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Book {
    pub me: Option<PcSettings>,
    pub peers: Vec<Peer>,
}

impl Book {
    pub fn load(home: &Home) -> Result<Self> {
        match std::fs::read(path(home)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err.into()),
        }
    }

    fn save(&self, home: &Home) -> Result<()> {
        let target = path(home);
        let temp = target.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temp, &target)?;
        Ok(())
    }

    pub fn get_mut(&mut self, pc: &PcId) -> Option<&mut Peer> {
        self.peers.iter_mut().find(|p| &p.pc == pc)
    }

    pub fn upsert(&mut self, peer: Peer) {
        match self.get_mut(&peer.pc) {
            Some(existing) => *existing = peer,
            None => self.peers.push(peer),
        }
    }
}

fn path(home: &Home) -> PathBuf {
    home.root().join("peers.json")
}

impl Inner {
    pub fn read_book<T>(&self, f: impl FnOnce(&mut Book) -> T) -> T {
        let mut book = self.book.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut book)
    }

    pub fn edit_book<T>(&self, f: impl FnOnce(&mut Book) -> T) -> Result<T> {
        let mut book = self.book.lock().unwrap_or_else(|e| e.into_inner());
        let out = f(&mut book);
        book.save(&self.home)?;
        Ok(out)
    }

    pub fn peers(&self) -> Vec<Peer> {
        self.read_book(|b| b.peers.clone())
    }

    pub fn peer(&self, pc: &PcId) -> Result<Peer> {
        self.read_book(|b| b.get_mut(pc).cloned()).ok_or_else(|| Error::not_found(format!("unknown pc {pc}")))
    }

    pub fn authorize(&self, pc: &PcId, key: &str) -> bool {
        self.read_book(|b| b.get_mut(pc).is_some_and(|p| same(&p.accept_key, key)))
    }
}
