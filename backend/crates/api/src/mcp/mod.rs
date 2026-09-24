//! Stateless MCP transport. Credentials are rechecked for every HTTP request
//! and again by the common executor; no session caches an authorization grant.
mod contract;
mod server;

use crate::{console_identity::secrets, routes::AppState};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::sync::Arc;

pub(crate) async fn handle(State(state): State<AppState>, request: Request) -> Response {
    let response = serve(state, request).await;
    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    parts.headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    Response::from_parts(parts, body)
}

async fn serve(state: AppState, request: Request) -> Response {
    // MCP is a machine-only entry point. Browser sessions stay on the console.
    if request.headers().contains_key(header::ORIGIN) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let hash = crate::resource_commands::token(request.headers())
        .map(secrets::token_hash)
        .unwrap_or_default();
    if let Err((id, error)) =
        grove_management_service::resources::admit_mcp(&state.pool, &hash).await
    {
        let status = if error.code == grove_management_command::ErrorCode::Unauthorized {
            StatusCode::UNAUTHORIZED
        } else if error.code == grove_management_command::ErrorCode::Forbidden {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        let mut response = (
            status,
            axum::Json(serde_json::json!({
                "request_id": id, "error": error,
            })),
        )
            .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Bearer realm=\"grove-management\""),
            );
        }
        return response;
    }
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(allowed_hosts(&state));
    let service = StreamableHttpService::new(
        move || Ok(server::Server::new(state.clone(), hash.clone())),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    service.handle(request).await.map(Body::new).into_response()
}

fn allowed_hosts(state: &AppState) -> Vec<String> {
    let mut hosts = vec!["localhost".into(), "127.0.0.1".into(), "[::1]".into()];
    if let Some(authority) = state
        .public_url
        .as_ref()
        .and_then(|url| url.parse::<axum::http::Uri>().ok())
        .and_then(|uri| uri.authority().cloned())
    {
        hosts.push(authority.as_str().to_owned());
    }
    hosts
}

#[cfg(test)]
mod tests;
