use std::sync::Arc;

use async_trait::async_trait;
use orrin_core::api::{
    Event, Find, FindOutput, Outline, OutlineOutput, Read, ReadOutput, Search, SearchOutput, Where, WhereOutput,
};
use orrin_core::service::{Index, Service, Setup};
use orrin_core::{Clock, Events, ProjectId, Result, ViewId};
use tokio::sync::broadcast::error::RecvError;

use crate::engine::Engine;
use crate::query;
use crate::records::Records;

pub struct IndexService {
    engine: Arc<Engine>,
    events: Events,
}

impl IndexService {
    pub fn open(setup: &Setup) -> Result<Self> {
        let dir = setup.home.dir("index")?;
        let records = Records::open(&dir.join("index.db"))?;
        Ok(Self { engine: Arc::new(Engine::new(setup.hub.clone(), records)), events: setup.events.clone() })
    }
}

impl Service for IndexService {
    fn start(&self) -> Result<()> {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return Ok(()) };
        let engine = self.engine.clone();
        let mut events = self.events.subscribe();
        runtime.spawn(async move {
            loop {
                match events.recv().await {
                    Ok(envelope) => {
                        if let Event::ViewLanded { view, .. } = envelope.event {
                            engine.forget(&view);
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        });
        let engine = self.engine.clone();
        runtime.spawn(async move {
            if let Err(err) = engine.warm().await {
                tracing::warn!("index warm-up failed: {err}");
            }
        });
        Ok(())
    }
}

#[async_trait]
impl Index for IndexService {
    async fn find(&self, input: Find) -> Result<FindOutput> {
        query::find(&self.engine, input).await
    }

    async fn search(&self, input: Search) -> Result<SearchOutput> {
        query::search(&self.engine, input).await
    }

    async fn outline(&self, input: Outline) -> Result<OutlineOutput> {
        query::outline(&self.engine, input).await
    }

    async fn locate(&self, input: Where) -> Result<WhereOutput> {
        query::locate(&self.engine, input).await
    }

    async fn touched(&self, project: &ProjectId, view: Option<&ViewId>, paths: &[String]) -> Result<Clock> {
        self.engine.touched(project, view, paths).await
    }

    async fn clock(&self, project: &ProjectId, view: Option<&ViewId>) -> Clock {
        self.engine.clock(project, view).await
    }

    async fn read_anchor(&self, input: Read) -> Result<ReadOutput> {
        query::read_anchor(&self.engine, input).await
    }
}
