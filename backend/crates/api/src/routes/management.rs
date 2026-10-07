//! Account and registry management. Authentication stays on each management surface.

use axum::{
    Router,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{any, post},
};

use super::AppState;

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/v1", any(legacy_removed))
        .route("/api/admin/v1/", any(legacy_removed))
        .route("/api/admin/v1/{*path}", any(legacy_removed))
        .merge(crate::openapi::routes())
        .route("/api/admin/mcp", any(crate::mcp::handle))
        .route("/api/admin/commands/v1", post(crate::commands::execute))
        .merge(crate::accounts::resources::routes(state.clone()))
        .nest("/api/admin/identity/v1", crate::accounts::routes(state))
}

async fn legacy_removed() -> impl IntoResponse {
    (
        StatusCode::GONE,
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({"error":"legacy_admin_removed"})),
    )
}
