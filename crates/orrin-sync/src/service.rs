use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orrin_core::api::{
    Done, Hashes, Objects, PcsList, PcsListOutput, PcsPair, PcsPairOutput, PcsRedeem, PcsSettings, Stored,
    SyncAnnounce, SyncChanges, SyncChangesOutput, SyncGet, SyncMissing, SyncPair, SyncPairOutput, SyncPoke, SyncPut,
};
use orrin_core::model::{Pc, PcSettings};
use orrin_core::service::{Hub, Peers, Service, Setup};
use orrin_core::{Config, Events, Hash, Home, Identity, PcId, ProjectId, Result};
use serde_json::Value;

use crate::book::Book;
use crate::pairing::Pending;
use crate::poke::{self, Pokes};
use crate::rounds;
use crate::transport::Transport;

pub struct PeerService(Arc<Inner>);

pub(crate) struct Inner {
    pub hub: Hub,
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub events: Events,
    pub book: Mutex<Book>,
    pub pending: Mutex<Vec<Pending>>,
    pub following: tokio::sync::Mutex<()>,
    pub pokes: Pokes,
    pub transport: Transport,
}

impl PeerService {
    pub fn open(setup: &Setup) -> Result<Self> {
        Ok(Self(Arc::new(Inner {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            events: setup.events.clone(),
            book: Mutex::new(Book::load(&setup.home)?),
            pending: Mutex::new(Vec::new()),
            following: tokio::sync::Mutex::new(()),
            pokes: Pokes::default(),
            transport: Transport::default(),
        })))
    }
}

impl Service for PeerService {
    fn start(&self) -> Result<()> {
        rounds::spawn(self.0.clone());
        poke::spawn(self.0.clone());
        Ok(())
    }
}

#[async_trait]
impl Peers for PeerService {
    async fn pcs(&self, _: PcsList) -> Result<PcsListOutput> {
        Ok(self.0.pcs())
    }

    async fn pair(&self, _: PcsPair) -> Result<PcsPairOutput> {
        self.0.pair()
    }

    async fn redeem(&self, input: PcsRedeem) -> Result<Pc> {
        self.0.redeem(input).await
    }

    async fn settings(&self, input: PcsSettings) -> Result<PcSettings> {
        self.0.settings(input)
    }

    async fn accept_pair(&self, input: SyncPair) -> Result<SyncPairOutput> {
        self.0.accept_pair(input)
    }

    async fn announce(&self, input: SyncAnnounce) -> Result<SyncAnnounce> {
        self.0.announce(input).await
    }

    async fn missing(&self, input: SyncMissing) -> Result<Hashes> {
        Ok(self.0.missing(input))
    }

    async fn objects(&self, input: SyncGet) -> Result<Objects> {
        Ok(self.0.objects(input))
    }

    async fn receive(&self, input: SyncPut) -> Result<Stored> {
        self.0.receive(input)
    }

    async fn changes(&self, input: SyncChanges) -> Result<SyncChangesOutput> {
        self.0.changes(input).await
    }

    async fn poke(&self, input: SyncPoke) -> Result<Done> {
        self.0.peer(&input.pc)?;
        Ok(poke::poked(&self.0, input))
    }

    async fn authorize(&self, pc: &PcId, key: &str) -> bool {
        self.0.authorize(pc, key)
    }

    async fn call(&self, pc: &PcId, request: &str, body: Value) -> Result<Value> {
        self.0.call(pc, request, body).await
    }

    async fn pull(&self, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
        self.0.pull(pc, hashes).await
    }

    async fn push(&self, pc: &PcId, hashes: Vec<Hash>) -> Result<()> {
        self.0.push(pc, hashes).await
    }

    async fn refresh(&self, pc: &PcId, project: &ProjectId) -> Result<()> {
        self.0.refresh(pc, project).await
    }
}
