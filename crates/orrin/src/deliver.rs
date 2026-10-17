use std::time::Duration;

use orrin_core::api::{Deliver, DeliveryStatus};
use orrin_core::model::{Delivery, DeliveryState};

use crate::api::Api;
use crate::exit::Outcome;

const POLL: Duration = Duration::from_millis(300);

pub async fn deliver(api: &Api, project: Option<&str>, status: bool) -> anyhow::Result<Outcome> {
    let project = api.project(project).await?.id;
    let mut last = match status {
        true => api.send(DeliveryStatus { project: project.clone() }).await?,
        false => api.send(Deliver { project: project.clone() }).await?,
    };
    api.show(&last, show)?;
    while !status && !settled(last.state) {
        tokio::time::sleep(POLL).await;
        let now = api.send(DeliveryStatus { project: project.clone() }).await?;
        if now != last {
            api.show(&now, show)?;
        }
        last = now;
    }
    Ok(if last.state == DeliveryState::Failed { Outcome::Failed } else { Outcome::Done })
}

fn settled(state: DeliveryState) -> bool {
    matches!(state, DeliveryState::Done | DeliveryState::Failed | DeliveryState::Waiting)
}

fn show(delivery: &Delivery) -> String {
    match delivery.state {
        DeliveryState::Building => format!("building {}", delivery.targets.join(", ")),
        DeliveryState::Copying => "copying into the user folder".into(),
        DeliveryState::Waiting => format!("waiting for {} to close", delivery.waiting.join(", ")),
        DeliveryState::Done => format!("delivered: {} copied, {} unchanged", delivery.copied, delivery.unchanged),
        DeliveryState::Failed => format!("failed: {}", delivery.error.as_deref().unwrap_or("unknown error")),
    }
}
