use axum::Json;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use orrin_core::{Error, ErrorCode, Result};
use serde_json::{Value, json};

use crate::http::AppState;
use crate::http::auth::{allow, allow_local, identify};
use crate::transport::Remote;

pub async fn call(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<Remote>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let caller = identify(&state, remote, &headers, None).await;
    let result = match allow(&caller, &name).and_then(|_| parse(&body)) {
        Ok(input) => state.registry.call(state.hub.clone(), &name, input).await,
        Err(err) => Err(err),
    };
    reply(result)
}

pub async fn list(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<Remote>,
    headers: HeaderMap,
) -> Response {
    let caller = identify(&state, remote, &headers, None).await;
    reply(allow_local(&caller).map(|_| json!(state.registry.names())))
}

fn parse(body: &[u8]) -> Result<Value> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    serde_json::from_slice(body).map_err(|e| Error::invalid(format!("body is not JSON: {e}")))
}

fn reply(result: Result<Value>) -> Response {
    match result {
        Ok(result) => Json(json!({ "ok": true, "result": result })).into_response(),
        Err(error) => (status(error.code), Json(json!({ "ok": false, "error": error }))).into_response(),
    }
}

pub fn status(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::Invalid => StatusCode::BAD_REQUEST,
        ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
