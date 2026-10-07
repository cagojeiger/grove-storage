//! File operations authenticate Clients or leases, independently of management accounts.

use axum::{Router, middleware};

use super::AppState;

pub(super) fn control(state: AppState) -> Router<AppState> {
    Router::new().nest(
        "/api/v1",
        crate::native::routes().route_layer(middleware::from_fn_with_state(
            state,
            crate::native::require_client,
        )),
    )
}

// Streaming paths keep their existing limits; they do not inherit the JSON body limit.
pub(super) fn streaming(cors_allowed_origins: &[String]) -> Router<AppState> {
    Router::new()
        .nest("/blobs", crate::lease_relay::routes(cors_allowed_origins))
        .merge(crate::s3::routes(cors_allowed_origins))
}
