use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use orrin_core::Events;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::http::AppState;
use crate::http::auth::{allow_local, identify};
use crate::transport::Remote;

#[derive(Deserialize)]
pub struct KeyQuery {
    key: Option<String>,
}

pub async fn subscribe(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<Remote>,
    Query(query): Query<KeyQuery>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if allow_local(&identify(&state, remote, &headers, query.key.as_deref()).await).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let events = state.hub.events.clone();
    upgrade.on_upgrade(move |socket| forward(socket, events))
}

async fn forward(mut socket: WebSocket, events: Events) {
    let mut rx = events.subscribe();
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(envelope) => {
                    let Ok(text) = serde_json::to_string(&envelope) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                Some(Ok(_)) => continue,
            },
        }
    }
}
