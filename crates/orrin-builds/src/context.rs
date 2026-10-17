use orrin_core::api::PcsSettings;
use orrin_core::service::{Hub, Setup};
use orrin_core::{Config, Home, Identity, Result};

use crate::cache::Cache;
use crate::deliver::Deliveries;
use crate::journal::Journal;
use crate::linked::Links;
use crate::queue::Queue;
use crate::runner::warm::Pool;
use crate::table::Table;

pub struct Context {
    pub hub: Hub,
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub table: Table,
    pub queue: Queue,
    pub cache: Cache,
    pub warm: Pool,
    pub links: Links,
    pub deliveries: Deliveries,
}

impl Context {
    pub fn new(setup: &Setup) -> Result<Self> {
        Ok(Self {
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            table: Table::new(setup.events.clone(), Journal::open(&setup.home.db())?),
            queue: Queue::new(&setup.config.builds),
            cache: Cache::new(setup.home.dir("builds/cache")?),
            warm: Pool::default(),
            links: Links::new(setup.home.dir("builds/linked")?),
            deliveries: Deliveries::default(),
        })
    }

    pub async fn builds_on(&self) -> bool {
        let mine = PcsSettings { pc: None, set: None };
        self.hub.peers.settings(mine).await.map_or(self.config.pc.builds, |s| s.builds)
    }
}
