use std::net::SocketAddr;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use orrin_core::Events;
use tokio::sync::broadcast::error::RecvError;

use crate::http::AppState;
use crate::http::auth::{Caller, identify};

pub async fn subscribe(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if identify(&state.hub, addr, &headers).await == Caller::Stranger {
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
