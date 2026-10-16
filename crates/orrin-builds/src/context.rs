use orrin_core::api::PcsSettings;
use orrin_core::service::{Hub, Setup};
use orrin_core::{Config, Home, Identity, Result};

use crate::cache::Cache;
use crate::queue::Queue;
use crate::table::Table;

pub struct Context {
    pub hub: Hub,
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub table: Table,
    pub queue: Queue,
    pub cache: Cache,
}

impl Context {
    pub fn new(setup: &Setup) -> Result<Self> {
        Ok(Self {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            table: Table::new(setup.events.clone()),
            queue: Queue::new(&setup.config.builds),
            cache: Cache::new(setup.home.dir("builds/cache")?),
        })
    }

    pub async fn builds_on(&self) -> bool {
        let mine = PcsSettings { pc: None, set: None };
        self.hub.peers.settings(mine).await.map_or(self.config.pc.builds, |s| s.builds)
    }
}
