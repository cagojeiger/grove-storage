//! Standalone administrator authentication; runtime credentials use separate middleware.

pub mod cli;
mod session;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use filegate_db::admin_auth as db;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{ApiError, internal, status, unauthorized};
use crate::routes::AppState;

pub use session::routes;

const COOKIE: &str = "__Host-filegate_session";

pub fn hash(domain: &str, raw: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update([0]);
    digest.update(raw);
    hex::encode(digest.finalize())
}

fn valid_secret(raw: &str, prefix: &str) -> bool {
    raw.strip_prefix(prefix)
        .is_some_and(|suffix| suffix.len() == 64 && suffix.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn console_origin(value: Option<String>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let uri: axum::http::Uri = value.parse()?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.host().is_some()
            && uri.path() == "/"
            && uri.query().is_none()
            && !value.contains(['@', '#', '?'])
            && !value.ends_with('/'),
        "FILEGATE_CONSOLE_ORIGIN must be an HTTPS origin without a trailing slash"
    );
    Ok(Some(value))
}

fn csrf(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(origin) = state.console_origin.as_deref() else {
        return Err(status(StatusCode::NOT_FOUND, "console sessions disabled"));
    };
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(origin)
        || headers.get("x-filegate-csrf").and_then(|v| v.to_str().ok()) != Some("1")
    {
        return Err(status(
            StatusCode::FORBIDDEN,
            "same-origin request required",
        ));
    }
    Ok(())
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut cookies = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|v| v.trim().split_once('='))
        .filter(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value);
    let value = cookies.next()?;
    (cookies.next().is_none() && valid_secret(value, "fgss_")).then_some(value)
}

async fn cookie_actor(state: &AppState, headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let origin = state
        .console_origin
        .as_deref()
        .ok_or_else(|| unauthorized("operator token required"))?;
    if headers
        .get(header::ORIGIN)
        .is_some_and(|v| v.as_bytes() != origin.as_bytes())
        || headers
            .get("sec-fetch-site")
            .is_some_and(|v| v != "same-origin" && v != "none")
    {
        return Err(status(
            StatusCode::FORBIDDEN,
            "same-origin request required",
        ));
    }
    let raw = session_cookie(headers).ok_or_else(|| unauthorized("operator token required"))?;
    db::session_actor(&state.pool, &hash("admin-session", raw))
        .await?
        .ok_or_else(|| unauthorized("operator token required"))
}

async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
) -> Result<Option<Uuid>, ApiError> {
    if headers.contains_key(header::AUTHORIZATION) {
        let raw = crate::routes::bearer_token(headers)
            .ok_or_else(|| unauthorized("operator token required"))?;
        if valid_secret(raw, "fgop_")
            && let Some(id) = db::authenticate(&state.pool, &hash("admin-token", raw)).await?
        {
            return Ok(Some(id));
        }
        if state.security.operator_token_matches(raw) && !db::initialized(&state.pool).await? {
            return Ok(None);
        }
        return Err(unauthorized("operator token required"));
    }
    if session_cookie(headers).is_none() {
        return Err(unauthorized("operator token required"));
    }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        csrf(state, headers)?;
    }
    cookie_actor(state, headers).await.map(Some)
}

pub async fn require_operator(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let credential = match actor(&state, request.headers(), request.method()).await {
        Ok(actor) => actor,
        Err(error) => return error.into_response(),
    };
    let mutation = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let audit = if mutation {
        let target = request.uri().path().to_owned();
        let method = request.method().to_string();
        match db::audit_start(&state.pool, credential, &method, &target).await {
            Ok(id) => Some(id),
            Err(error) => return internal(error).into_response(),
        }
    } else {
        None
    };
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(id) = audit
        && let Err(error) = db::audit_finish(&state.pool, id, response.status().as_u16()).await
    {
        // The mutation may already have committed; leave the audit intent pending.
        tracing::error!(event = "admin.audit_incomplete", audit_id = id, %error);
    }
    response
}

#[cfg(test)]
mod tests;
