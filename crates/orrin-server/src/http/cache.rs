use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use orrin_core::Error;

use crate::http::AppState;
use crate::http::api::status;
use crate::http::auth::{allow_local, identify};
use crate::transport::Remote;

const COLLECTION: &str = concat!(
    r#"<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:"><D:response><D:href>{href}</D:href>"#,
    r#"<D:propstat><D:prop><D:getlastmodified>Thu, 01 Jan 1970 00:00:00 GMT</D:getlastmodified>"#,
    r#"<D:resourcetype><D:collection/></D:resourcetype></D:prop>"#,
    r#"<D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#,
);

pub async fn serve(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<Remote>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let bearer = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    let token = bearer.and_then(|v| v.strip_prefix("Bearer "));
    if allow_local(&identify(&state, remote, &headers, token).await).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = uri.path();
    let key = path.rsplit('/').next().unwrap_or_default();
    let cache = &state.hub.cache;
    match method.as_str() {
        "GET" | "HEAD" => match cache.get(key).await {
            Ok(Some(bytes)) => bytes.into_response(),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(err) => failed(err),
        },
        "PUT" => match cache.put(key, body.to_vec()).await {
            Ok(()) => StatusCode::CREATED.into_response(),
            Err(err) => failed(err),
        },
        "PROPFIND" if key.is_empty() => collection(path),
        "PROPFIND" => StatusCode::NOT_FOUND.into_response(),
        "MKCOL" => StatusCode::CREATED.into_response(),
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn collection(path: &str) -> Response {
    let xml = COLLECTION.replace("{href}", path);
    (StatusCode::MULTI_STATUS, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], xml).into_response()
}

fn failed(err: Error) -> Response {
    (status(err.code), err.to_string()).into_response()
}
