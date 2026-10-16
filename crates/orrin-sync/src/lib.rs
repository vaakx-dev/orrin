mod address;
mod announce;
mod book;
mod changes;
mod client;
mod follow;
mod objects;
mod offer;
mod pairing;
mod poke;
mod presence;
mod rounds;
mod secret;
mod service;
mod settings;
mod space;

use std::sync::Arc;

use orrin_core::Result;
use orrin_core::service::{Peers, Setup};

pub fn open(setup: &Setup) -> Result<Arc<dyn Peers>> {
    Ok(Arc::new(service::PeerService::open(setup)?))
}
