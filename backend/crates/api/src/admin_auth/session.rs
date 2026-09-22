use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::Utc;
use filegate_core::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::*;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/session",
        axum::routing::post(login).get(current).delete(logout),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    token: SecretString,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response, ApiError> {
    csrf(&state, &headers)?;
    if !db::login_allowed(&state.pool).await? {
        let mut response =
            status(StatusCode::TOO_MANY_REQUESTS, "login rate limit exceeded").into_response();
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        return Ok(response);
    }
    if !valid_secret(input.token.expose_secret(), "fgop_") {
        return Err(unauthorized("operator token required"));
    }
    let raw = format!("fgss_{}", filegate_core::generate_url_secret());
    let (credential, expires_at) = db::create_session(
        &state.pool,
        &hash("admin-token", input.token.expose_secret()),
        &hash("admin-session", &raw),
    )
    .await?
    .ok_or_else(|| unauthorized("operator token required"))?;
    if let Some(previous) = session_cookie(&headers) {
        db::logout(&state.pool, &hash("admin-session", previous)).await?;
    }
    let age = (expires_at - Utc::now()).num_seconds().max(0);
    let mut response = Json(serde_json::json!({"principal": "admin", "credential_id": credential, "expires_at": expires_at})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={raw}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={age}"
        ))
        .map_err(internal)?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn current(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let credential = cookie_actor(&state, &headers).await?;
    let mut response =
        Json(serde_json::json!({"principal":"admin", "credential_id":credential})).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    csrf(&state, &headers)?;
    if let Some(raw) = session_cookie(&headers) {
        db::logout(&state.pool, &hash("admin-session", raw)).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "__Host-filegate_session=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
