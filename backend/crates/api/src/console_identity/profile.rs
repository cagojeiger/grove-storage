use axum::{
    Json,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use grove_management_service::{self as service, Error};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    failure, identified,
    inputs::{self, Body},
    output, personal_tokens,
};
use crate::routes::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rename {
    display_name: String,
}

pub(super) async fn get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = Uuid::new_v4();
    let session_hash = match personal_tokens::session(&headers) {
        Ok(hash) => hash,
        Err(error) => return failure(error, request_id),
    };
    match service::profile::get(&state.pool, &session_hash).await {
        Ok(row) => identified(Json(output::account(row)).into_response(), request_id),
        Err(error) => failure(error, request_id),
    }
}

pub(super) async fn rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<Rename>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(body)) = body else {
        return inputs::invalid();
    };
    if !inputs::valid_label(&body.display_name) {
        return failure(Error::InvalidInput, request_id);
    }
    let session_hash = match personal_tokens::session(&headers) {
        Ok(hash) => hash,
        Err(error) => return failure(error, request_id),
    };
    match service::profile::rename(&state.pool, request_id, &session_hash, body.display_name).await
    {
        Ok(changed) => identified(
            Json(serde_json::json!({"changed": changed})).into_response(),
            request_id,
        ),
        Err(error) => failure(error, request_id),
    }
}
