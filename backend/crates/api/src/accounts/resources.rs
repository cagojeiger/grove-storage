//! Cookie-only adapter for the shared resource command executor.
use super::browser;
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
    let hash = browser::session_hash(&headers);
    crate::commands::execute_with(
        state,
        grove_management_service::Proof::PasswordSession(&hash),
        Surface::Console,
        body,
    )
    .await
}
