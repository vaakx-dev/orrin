use orrin_core::service::{Hub, Services, Setup};
use orrin_core::{Config, Events, Home, Identity, Result};

pub fn wire(home: Home, config: Config) -> Result<Hub> {
    let me = Identity::load_or_create(&home)?;
    let setup = Setup { home, config, me, events: Events::new(), hub: Hub::new() };
    let services = Services {
        home: setup.home.clone(),
        config: setup.config.clone(),
        me: setup.me.clone(),
        events: setup.events.clone(),
        store: orrin_store::open(&setup)?,
        projects: orrin_projects::open(&setup)?,
        views: orrin_views::open(&setup)?,
        landing: orrin_landing::open(&setup)?,
        mergers: orrin_merge::all(),
        index: orrin_index::open(&setup)?,
        facts: orrin_facts::open(&setup)?,
        builds: orrin_builds::open(&setup)?,
        peers: orrin_sync::open(&setup)?,
        git: orrin_git::open(&setup)?,
    };
    setup.hub.set(services);
    start(&setup.hub)?;
    crate::maintenance::start(&setup.hub);
    Ok(setup.hub)
}

fn start(hub: &Hub) -> Result<()> {
    hub.store.start()?;
    hub.projects.start()?;
    hub.views.start()?;
    hub.landing.start()?;
    hub.index.start()?;
    hub.facts.start()?;
    hub.builds.start()?;
    hub.peers.start()?;
    hub.git.start()
}
