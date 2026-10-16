use async_trait::async_trait;
use serde_json::Value;

use crate::api::{
    Hashes, Objects, PcsList, PcsListOutput, PcsPair, PcsPairOutput, PcsRedeem, PcsSettings, Request, Stored,
    SyncAnnounce, SyncChanges, SyncChangesOutput, SyncGet, SyncMissing, SyncPair, SyncPairOutput, SyncPut,
};
use crate::error::Result;
use crate::id::PcId;
use crate::model::{Pc, PcSettings};
use crate::service::Service;

#[async_trait]
pub trait Peers: Service {
    async fn pcs(&self, input: PcsList) -> Result<PcsListOutput>;
    async fn pair(&self, input: PcsPair) -> Result<PcsPairOutput>;
    async fn redeem(&self, input: PcsRedeem) -> Result<Pc>;
    async fn settings(&self, input: PcsSettings) -> Result<PcSettings>;
    async fn accept_pair(&self, input: SyncPair) -> Result<SyncPairOutput>;
    async fn announce(&self, input: SyncAnnounce) -> Result<SyncAnnounce>;
    async fn missing(&self, input: SyncMissing) -> Result<Hashes>;
    async fn objects(&self, input: SyncGet) -> Result<Objects>;
    async fn receive(&self, input: SyncPut) -> Result<Stored>;
    async fn changes(&self, input: SyncChanges) -> Result<SyncChangesOutput>;
    async fn authorize(&self, pc: &PcId, key: &str) -> bool;
    async fn call(&self, pc: &PcId, request: &str, body: Value) -> Result<Value>;
}

pub async fn call_peer<R: Request>(peers: &dyn Peers, pc: &PcId, input: R) -> Result<R::Output> {
    let body = serde_json::to_value(input)?;
    let value = peers.call(pc, R::NAME, body).await?;
    Ok(serde_json::from_value(value)?)
}
