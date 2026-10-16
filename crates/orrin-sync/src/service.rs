use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use orrin_core::api::{
    Hashes, Objects, PcsList, PcsListOutput, PcsPair, PcsPairOutput, PcsRedeem, PcsSettings, Stored, SyncAnnounce,
    SyncChanges, SyncChangesOutput, SyncGet, SyncMissing, SyncPair, SyncPairOutput, SyncPut,
};
use orrin_core::model::{Pc, PcSettings};
use orrin_core::service::{Hub, Peers, Service, Setup};
use orrin_core::{Config, Error, Events, Hash, Home, Identity, PcId, Result};
use serde_json::Value;

use crate::book::Book;
use crate::pairing::Pending;
use crate::rounds;

pub struct PeerService(Arc<Inner>);

pub(crate) struct Inner {
    pub hub: Hub,
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub events: Events,
    pub book: Mutex<Book>,
    pub pending: Mutex<Vec<Pending>>,
    pub http: reqwest::Client,
}

impl PeerService {
    pub fn open(setup: &Setup) -> Result<Self> {
        let http = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| Error::internal(format!("http client: {e}")))?;
        Ok(Self(Arc::new(Inner {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            events: setup.events.clone(),
            book: Mutex::new(Book::load(&setup.home)?),
            pending: Mutex::new(Vec::new()),
            http,
        })))
    }
}

impl Service for PeerService {
    fn start(&self) -> Result<()> {
        rounds::spawn(self.0.clone());
        Ok(())
    }
}

#[async_trait]
impl Peers for PeerService {
    async fn pcs(&self, _: PcsList) -> Result<PcsListOutput> {
        Ok(self.0.pcs())
    }

    async fn pair(&self, _: PcsPair) -> Result<PcsPairOutput> {
        Ok(self.0.pair())
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
}
