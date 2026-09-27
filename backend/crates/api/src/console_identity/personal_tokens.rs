use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use filegate_core::SecretString;
use grove_management_service::{self as service, Error};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    browser, failure, identified,
    inputs::{self, Body, Id, QueryPage},
    output, secrets,
};
use crate::routes::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Issue {
    label: String,
    expires_in_days: u16,
    current_password: SecretString,
}

pub(super) fn session(headers: &HeaderMap) -> Result<String, Error> {
    let (hash, root) = browser::session_hash(headers);
    if root || browser::cookie(headers).is_none() {
        return Err(Error::Unauthenticated);
    }
    Ok(hash)
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: QueryPage,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Query(query)) = query else {
        return inputs::invalid();
    };
    let Ok(page) = query.page() else {
        return inputs::invalid();
    };
    let session_hash = match session(&headers) {
        Ok(hash) => hash,
        Err(error) => return failure(error, request_id),
    };
    match service::personal_tokens::list(&state.pool, &session_hash, page).await {
        Ok(rows) => identified(
            Json(output::credential_page(rows, query.limit)).into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}

pub(super) async fn issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<Issue>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(body)) = body else {
        return inputs::invalid();
    };
    if !inputs::valid_label(&body.label) || !(1..=90).contains(&body.expires_in_days) {
        return inputs::invalid();
    }
    let session_hash = match session(&headers) {
        Ok(hash) => hash,
        Err(error) => return failure(error, request_id),
    };
    let token = secrets::IssuedToken::new();
    match service::personal_tokens::issue(
        &state.pool,
        request_id,
        &session_hash,
        body.current_password,
        &token.credential(body.label.trim(), body.expires_in_days),
    )
    .await
    {
        Ok(key) => identified(
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "account_id": key.account_id,
                    "credential_id": key.id,
                    "expires_at": key.expires_at,
                    "token": token.expose(),
                })),
            )
                .into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}

pub(super) async fn revoke(State(state): State<AppState>, headers: HeaderMap, id: Id) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Path(id)) = id else {
        return inputs::invalid();
    };
    let session_hash = match session(&headers) {
        Ok(hash) => hash,
        Err(error) => return failure(error, request_id),
    };
    match service::personal_tokens::revoke(&state.pool, request_id, &session_hash, id).await {
        Ok(changed) => identified(
            Json(serde_json::json!({"changed": changed})).into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}
