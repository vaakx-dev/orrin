mod code;

use orrin_core::api::{PcsPairOutput, PcsRedeem, Request, SyncPair, SyncPairOutput};
use orrin_core::model::{Pc, PcSettings};
use orrin_core::{Error, ErrorCode, Millis, Result, now_ms};

use crate::book::Peer;
use crate::secret::{same, secret};
use crate::service::Inner;
use code::PairCode;

const LIFETIME: Millis = 15 * 60 * 1000;

pub struct Pending {
    secret: String,
    expires_at: Millis,
}

impl Inner {
    pub fn pair(&self) -> PcsPairOutput {
        let secret = secret();
        let expires_at = now_ms() + LIFETIME;
        let code = PairCode {
            pc: self.me.pc.clone(),
            name: self.me.name.clone(),
            addresses: self.addresses(),
            secret: secret.clone(),
        };
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|p| p.expires_at > now_ms());
        pending.push(Pending { secret, expires_at });
        PcsPairOutput { code: code.encode(), expires_at }
    }

    pub async fn redeem(&self, input: PcsRedeem) -> Result<Pc> {
        let code = PairCode::decode(&input.code)?;
        if code.pc == self.me.pc {
            return Err(Error::invalid("that code was made on this PC"));
        }
        let accept_key = secret();
        let offer = SyncPair {
            code: code.secret.clone(),
            pc: self.me.pc.clone(),
            name: self.me.name.clone(),
            address: self.address(),
            key: accept_key.clone(),
        };
        let body = serde_json::to_value(&offer)?;
        let mut last = Error::invalid("the code has no addresses");
        for address in candidates(&input.address, &code.addresses) {
            match self.post(&address, SyncPair::NAME, &body, None).await {
                Ok(value) => {
                    let reply: SyncPairOutput = serde_json::from_value(value)?;
                    if reply.pc != code.pc {
                        return Err(Error::conflict(format!("{address} is {}, not {}", reply.pc, code.pc)));
                    }
                    return self.remember(Peer {
                        pc: reply.pc,
                        name: reply.name,
                        address,
                        send_key: reply.key,
                        accept_key,
                        settings: PcSettings::default(),
                        last_seen: Some(now_ms()),
                        online: true,
                    });
                }
                Err(err) if err.code != ErrorCode::Unavailable => return Err(err),
                Err(err) => last = err,
            }
        }
        Err(last)
    }

    pub fn accept_pair(&self, input: SyncPair) -> Result<SyncPairOutput> {
        if input.pc == self.me.pc {
            return Err(Error::invalid("a PC can't pair with itself"));
        }
        self.take_code(&input.code)?;
        let accept_key = secret();
        self.remember(Peer {
            pc: input.pc,
            name: input.name,
            address: input.address,
            send_key: input.key,
            accept_key: accept_key.clone(),
            settings: PcSettings::default(),
            last_seen: Some(now_ms()),
            online: true,
        })?;
        Ok(SyncPairOutput { pc: self.me.pc.clone(), name: self.me.name.clone(), key: accept_key })
    }

    fn take_code(&self, secret: &str) -> Result<()> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|p| p.expires_at > now_ms());
        let index = pending
            .iter()
            .position(|p| same(&p.secret, secret))
            .ok_or_else(|| Error::unauthorized("unknown or expired pairing code"))?;
        pending.remove(index);
        Ok(())
    }

    fn remember(&self, peer: Peer) -> Result<Pc> {
        let pc = peer.pc();
        self.edit_book(|b| b.upsert(peer))?;
        self.announce_seen(pc.clone());
        Ok(pc)
    }
}

fn candidates(first: &str, rest: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Some(first.trim().to_string()).filter(|a| !a.is_empty()).into_iter().collect();
    for address in rest {
        if !all.contains(address) {
            all.push(address.clone());
        }
    }
    all
}
