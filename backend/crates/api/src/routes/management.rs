//! Account and registry management. Authentication stays on each management surface.

use axum::{
    Router,
    http::{StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::{any, post},
};

use super::AppState;

pub(super) fn routes(state: AppState) -> Router<AppState> {
    let legacy = if state.security.legacy_admin_enabled {
        Router::new().nest(
            "/api/admin/v1",
            crate::admin::admin_routes()
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    crate::admin_auth::require_operator,
                ))
                .merge(crate::admin_auth::routes()),
        )
    } else {
        Router::new()
            .route("/api/admin/v1", any(legacy_disabled))
            .route("/api/admin/v1/", any(legacy_disabled))
            .route("/api/admin/v1/{*path}", any(legacy_disabled))
    };

    Router::new()
        .merge(legacy)
        .merge(crate::openapi::routes())
        .route("/api/admin/mcp", any(crate::mcp::handle))
        .route(
            "/api/admin/commands/v1",
            post(crate::resource_commands::execute),
        )
        .merge(crate::console_identity::resources::routes(state.clone()))
        .nest(
            "/api/admin/identity/v1",
            crate::console_identity::routes(state),
        )
}

async fn legacy_disabled() -> impl IntoResponse {
    (
        StatusCode::GONE,
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({"error":"legacy_admin_disabled"})),
    )
}
