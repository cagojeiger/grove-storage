use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use filegate_core::{ExposeSecret, SecretString};
use grove_management_service::{self as service, Error};
use serde::Deserialize;
use uuid::Uuid;

use super::{browser, failure, identified, secrets};
use crate::routes::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Issue {
    username: String,
    current_password: SecretString,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inspect {
    token: SecretString,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Complete {
    token: SecretString,
    password: SecretString,
}

pub(super) async fn issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(account): Path<Uuid>,
    body: Result<Json<Issue>, JsonRejection>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, request_id);
    };
    let (session_hash, root) = browser::session_hash(&headers);
    if root || browser::cookie(&headers).is_none() {
        return failure(Error::Unauthenticated, request_id);
    }
    let token = SecretString::from(format!(
        "{}{}",
        secrets::SETUP_PREFIX,
        filegate_core::generate_url_secret()
    ));
    let token_hash = secrets::setup_hash(token.expose_secret());
    match service::password_setups::issue(
        &state.pool,
        request_id,
        &session_hash,
        account,
        &body.username,
        body.current_password,
        &token_hash,
    )
    .await
    {
        Ok(setup) => identified(
            Json(serde_json::json!({
                "account_id": setup.account_id,
                "username": setup.login_name,
                "expires_at": setup.expires_at,
                "token": token.expose_secret(),
            }))
            .into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}

pub(super) async fn inspect(
    State(state): State<AppState>,
    body: Result<Json<Inspect>, JsonRejection>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, request_id);
    };
    if !secrets::valid(body.token.expose_secret(), secrets::SETUP_PREFIX) {
        return failure(Error::NotFound, request_id);
    }
    match service::password_setups::inspect(
        &state.pool,
        &secrets::setup_hash(body.token.expose_secret()),
    )
    .await
    {
        Ok(setup) => identified(
            Json(serde_json::json!({"username": setup.login_name, "expires_at": setup.expires_at}))
                .into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}

pub(super) async fn complete(
    State(state): State<AppState>,
    body: Result<Json<Complete>, JsonRejection>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, request_id);
    };
    if !secrets::valid(body.token.expose_secret(), secrets::SETUP_PREFIX) {
        return failure(Error::NotFound, request_id);
    }
    match service::password_setups::complete(
        &state.pool,
        request_id,
        &secrets::setup_hash(body.token.expose_secret()),
        body.password,
    )
    .await
    {
        Ok(()) => identified(StatusCode::NO_CONTENT.into_response(), request_id),
        Err(error) => failure(error, request_id),
    }
}
