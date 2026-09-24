use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use filegate_core::{ExposeSecret, SecretString};
use grove_management_policy::{Actor, Role, Surface};
use grove_management_service::{self as service, Command, Error, Output, Proof};
use serde::Deserialize;
use uuid::Uuid;

use super::{browser, failure, identified, secrets};
use crate::routes::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Login {
    token: SecretString,
}

pub(super) async fn login(
    State(state): State<AppState>,
    body: Result<Json<Login>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, Uuid::new_v4());
    };
    let hash = secrets::valid(body.token.expose_secret(), secrets::TOKEN_PREFIX)
        .then(|| secrets::token_hash(body.token.expose_secret()));
    let raw = SecretString::from(format!(
        "{}{}",
        secrets::SESSION_PREFIX,
        filegate_core::generate_url_secret()
    ));
    let result = service::sessions::login(
        &state.pool,
        hash.as_deref(),
        &secrets::session_hash(raw.expose_secret()),
    )
    .await;
    match result.result {
        Ok(session) => {
            let mut response = Json(serde_json::json!({"principal": "user", "user_id": session.user_id,
                "credential_id": session.credential_id, "session_id": session.id, "expires_at": session.expires_at})).into_response();
            browser::set_cookie(
                &mut response,
                raw.expose_secret(),
                (session.expires_at - chrono::Utc::now())
                    .num_seconds()
                    .max(0),
            );
            identified(response, result.request_id)
        }
        Err(error) => failure(error, result.request_id),
    }
}

pub(super) async fn current(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let execution = execute(&state, &headers, Command::CurrentSession).await;
    match execution.result {
        Ok(Output::Identity(identity)) => {
            let Actor::User { role, .. } = identity.caller.actor else {
                return failure(Error::Unavailable, execution.request_id);
            };
            let role = match role {
                Role::Viewer => "viewer",
                Role::Operator => "operator",
                Role::Admin => "admin",
            };
            identified(Json(serde_json::json!({"principal":"user", "user_id": identity.account_id,
                "credential_id":identity.credential_id, "session_id":identity.session_id, "role":role})).into_response(), execution.request_id)
        }
        Err(error) => failure(error, execution.request_id),
        Ok(_) => failure(Error::Unavailable, execution.request_id),
    }
}

pub(super) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let execution = execute(&state, &headers, Command::Logout).await;
    let mut response = match execution.result {
        Ok(Output::Changed(_)) => {
            identified(StatusCode::NO_CONTENT.into_response(), execution.request_id)
        }
        Err(Error::Unauthenticated) => failure(Error::Unauthenticated, execution.request_id),
        Err(error) => return failure(error, execution.request_id),
        Ok(_) => return failure(Error::Unavailable, execution.request_id),
    };
    browser::set_cookie(&mut response, "", 0);
    response
}

async fn execute(
    state: &AppState,
    headers: &HeaderMap,
    command: Command<'_>,
) -> service::Execution {
    let hash = browser::cookie(headers)
        .map(secrets::session_hash)
        .unwrap_or_default();
    service::execute(
        &state.pool,
        Proof::Session(&hash),
        Surface::Console,
        command,
    )
    .await
}
