use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::post;
use orrin_core::daemon::DaemonInfo;
use orrin_core::service::{Hub, Services, Setup, Unavailable};
use orrin_core::{Config, Error, Events, Home, Identity, PcId, Result, now_ms};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::projects::MemoryProjects;
use super::store::MemoryStore;
use super::tls::TlsListener;

pub struct Node {
    pub hub: Hub,
    pub home: Home,
    _dir: tempfile::TempDir,
}

impl Node {
    pub async fn start(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let me = Identity { pc: PcId::new(), name: name.into(), key: "unused".into() };
        DaemonInfo { port, pid: std::process::id(), pc: me.pc.clone(), started_at: now_ms() }.write(&home).unwrap();
        let mut config = Config::default();
        config.server.bind = "127.0.0.1".into();
        let setup = Setup { home: home.clone(), config, me, events: Events::new(), hub: Hub::new() };
        let missing = Arc::new(Unavailable("fake"));
        setup.hub.set(Services {
            home: setup.home.clone(),
            config: setup.config.clone(),
            me: setup.me.clone(),
            events: setup.events.clone(),
            store: Arc::new(MemoryStore::default()),
            projects: Arc::new(MemoryProjects::new(setup.events.clone())),
            views: missing.clone(),
            landing: missing.clone(),
            mergers: Vec::new(),
            index: missing.clone(),
            facts: missing.clone(),
            builds: missing.clone(),
            cache: missing.clone(),
            peers: orrin_sync::open(&setup).unwrap(),
            git: missing,
        });
        let router = Router::new().route("/api/{name}", post(handle)).with_state(setup.hub.clone());
        let listener = TlsListener::new(listener, &home);
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { hub: setup.hub, home, _dir: dir }
    }
}

async fn handle(State(hub): State<Hub>, Path(name): Path<String>, Json(body): Json<Value>) -> Json<Value> {
    Json(match dispatch(&hub, &name, body).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    })
}

async fn dispatch(hub: &Hub, name: &str, body: Value) -> Result<Value> {
    let peers = &hub.peers;
    match name {
        "sync.pair" => out(peers.accept_pair(input(body)?).await),
        "sync.announce" => out(peers.announce(input(body)?).await),
        "sync.missing" => out(peers.missing(input(body)?).await),
        "sync.get" => out(peers.objects(input(body)?).await),
        "sync.put" => out(peers.receive(input(body)?).await),
        "sync.changes" => out(peers.changes(input(body)?).await),
        "sync.poke" => out(peers.poke(input(body)?).await),
        _ => Err(Error::not_found(name.to_string())),
    }
}

fn input<T: DeserializeOwned>(body: Value) -> Result<T> {
    Ok(serde_json::from_value(body)?)
}

fn out<T: Serialize>(result: Result<T>) -> Result<Value> {
    Ok(serde_json::to_value(result?)?)
}
