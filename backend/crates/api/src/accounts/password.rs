use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use grove_core::SecretString;
use grove_management_service::{self as service, Error};
use serde::Deserialize;
use uuid::Uuid;

use super::{browser, failure, identified};
use crate::routes::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PasswordChange {
    current_password: SecretString,
    new_password: SecretString,
}

pub(super) async fn change(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<PasswordChange>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, Uuid::new_v4());
    };
    let hash = browser::session_hash(&headers);
    if browser::cookie(&headers).is_none() {
        return failure(Error::Unauthenticated, Uuid::new_v4());
    }
    let result = service::password_changes::change(
        &state.pool,
        &hash,
        body.current_password,
        body.new_password,
    )
    .await;
    match result.result {
        Ok(()) => {
            let mut response =
                identified(StatusCode::NO_CONTENT.into_response(), result.request_id);
            browser::set_cookie(&mut response, "", 0);
            response
        }
        Err(error) => failure(error, result.request_id),
    }
}
