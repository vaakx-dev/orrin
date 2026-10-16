mod api;
pub mod auth;
mod events;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use orrin_core::service::Hub;

use crate::registry::Registry;

#[derive(Clone)]
pub struct AppState {
    pub hub: Hub,
    pub registry: Arc<Registry>,
    pub local_key: Arc<str>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api", get(api::list))
        .route("/api/{name}", post(api::call))
        .route("/events", get(events::subscribe))
        .with_state(state)
}
