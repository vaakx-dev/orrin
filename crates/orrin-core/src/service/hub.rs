use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use crate::config::Config;
use crate::events::Events;
use crate::home::Home;
use crate::identity::Identity;
use crate::service::{Builds, Facts, Git, Index, Landing, Merger, Peers, Projects, Store, Views};

pub struct Services {
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub events: Events,
    pub store: Arc<dyn Store>,
    pub projects: Arc<dyn Projects>,
    pub views: Arc<dyn Views>,
    pub landing: Arc<dyn Landing>,
    pub mergers: Vec<Arc<dyn Merger>>,
    pub index: Arc<dyn Index>,
    pub facts: Arc<dyn Facts>,
    pub builds: Arc<dyn Builds>,
    pub peers: Arc<dyn Peers>,
    pub git: Arc<dyn Git>,
}

impl Services {
    pub fn merger_for(&self, path: &str) -> Option<&Arc<dyn Merger>> {
        self.mergers.iter().find(|m| m.handles(path))
    }
}

#[derive(Clone, Default)]
pub struct Hub(Arc<OnceLock<Services>>);

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, services: Services) {
        if self.0.set(services).is_err() {
            panic!("hub already set");
        }
    }

    pub fn ready(&self) -> bool {
        self.0.get().is_some()
    }
}

impl Deref for Hub {
    type Target = Services;

    fn deref(&self) -> &Services {
        self.0.get().expect("hub used before the server finished wiring services")
    }
}

#[derive(Clone)]
pub struct Setup {
    pub home: Home,
    pub config: Config,
    pub me: Identity,
    pub events: Events,
    pub hub: Hub,
}
