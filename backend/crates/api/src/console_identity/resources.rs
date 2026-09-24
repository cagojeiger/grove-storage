//! Cookie-only adapter for the shared resource command executor.
use super::{browser, secrets};
use crate::routes::AppState;
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
    middleware,
    response::Response,
    routing::post,
};
use grove_management_policy::Surface;
use grove_management_service::Proof;

pub(crate) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/console-commands/v1", post(execute))
        .layer(middleware::from_fn_with_state(state, browser::guard))
}

async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Response {
    let hash = browser::cookie(&headers)
        .map(secrets::session_hash)
        .unwrap_or_default();
    crate::resource_commands::execute_with(state, Proof::Session(&hash), Surface::Console, body)
        .await
}
