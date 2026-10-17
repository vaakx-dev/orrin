mod claims;
mod commit;
mod into;
mod proposal;
mod rebase;
mod remote;
mod settle;
mod undo;

use std::sync::Arc;

use async_trait::async_trait;
use orrin_core::api::{LandOutput, SyncLand, Undo, ViewLand};
use orrin_core::model::ViewState;
use orrin_core::service::{Hub, Landing, Service, Setup};
use orrin_core::{Error, Result};
use tokio::sync::Mutex;

use crate::proposal::Proposal;

pub fn open(setup: &Setup) -> Result<Arc<dyn Landing>> {
    Ok(Arc::new(Lander { hub: setup.hub.clone(), turn: Mutex::new(()) }))
}

struct Lander {
    hub: Hub,
    turn: Mutex<()>,
}

impl Service for Lander {}

#[async_trait]
impl Landing for Lander {
    async fn land(&self, input: ViewLand) -> Result<LandOutput> {
        let _turn = self.turn.lock().await;
        let view = self.hub.views.get(&input.view).await?;
        if view.state != ViewState::Open {
            return Err(Error::invalid(format!("view {} is not open", view.id)));
        }
        let files = self.hub.views.capture(&view.id).await?;
        if let Some(into) = &input.into {
            return into::land_into(&self.hub, &view, files, input.intent, into).await;
        }
        let out = self.submit(Proposal::from_view(&view, files, input.intent)).await?;
        Ok(settle::settle(&self.hub, &view.project, Some(&view), out).await)
    }

    async fn undo(&self, input: Undo) -> Result<LandOutput> {
        let _turn = self.turn.lock().await;
        let project = self.hub.projects.get(&input.project).await?;
        let out = if project.home_pc == self.hub.me.pc {
            commit::commit(&self.hub, &undo::proposal(&self.hub, &input).await?).await?
        } else {
            remote::forward(&self.hub, &project.home_pc, input).await?
        };
        Ok(settle::settle(&self.hub, &project.id, None, out).await)
    }

    async fn land_change(&self, input: SyncLand) -> Result<LandOutput> {
        let _turn = self.turn.lock().await;
        let project = input.project.clone();
        let out = self.submit(Proposal::from_sync(input)).await?;
        Ok(settle::settle(&self.hub, &project, None, out).await)
    }
}

impl Lander {
    async fn submit(&self, proposal: Proposal) -> Result<LandOutput> {
        let project = self.hub.projects.get(&proposal.project).await?;
        if project.home_pc == self.hub.me.pc {
            return commit::commit(&self.hub, &proposal).await;
        }
        self.hub.peers.push(&project.home_pc, proposal.hashes()).await?;
        remote::forward(&self.hub, &project.home_pc, proposal.into_sync()).await
    }
}
