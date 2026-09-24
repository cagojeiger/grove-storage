//! New console identity surface. Legacy operator authentication stays separate
//! until the explicit migration; neither cookie grants the other's authority.
mod browser;
mod master;
pub(crate) mod master_config;
mod secrets;
mod session;

use axum::{
    Json, Router,
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::post,
};
use grove_management_service::Error;
use uuid::Uuid;

use crate::routes::AppState;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/session",
            post(session::login)
                .get(session::current)
                .delete(session::logout),
        )
        .route(
            "/master/session",
            post(master::login)
                .get(master::current)
                .delete(master::logout),
        )
        .route("/master/bootstrap", post(master::bootstrap))
        .route("/master/recover", post(master::recover))
        .layer(middleware::from_fn_with_state(state, browser::guard))
}

fn failure(error: Error, request_id: Uuid) -> Response {
    let status = match error {
        Error::Unauthenticated => StatusCode::UNAUTHORIZED,
        Error::Forbidden => StatusCode::FORBIDDEN,
        Error::NotFound => StatusCode::NOT_FOUND,
        Error::Conflict => StatusCode::CONFLICT,
        Error::InvalidInput => StatusCode::BAD_REQUEST,
        Error::Unavailable | Error::OutcomeUnknown => StatusCode::SERVICE_UNAVAILABLE,
        Error::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    };
    let mut response = (
        status,
        Json(serde_json::json!({"error": error.code(), "request_id": request_id})),
    )
        .into_response();
    if error == Error::RateLimited {
        response.headers_mut().insert(
            header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("60"),
        );
    }
    identified(response, request_id)
}

fn identified(mut response: Response, id: Uuid) -> Response {
    if let Ok(value) = id.to_string().parse() {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
mod tests;
