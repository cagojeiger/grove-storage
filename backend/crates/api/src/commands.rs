//! Shared HTTP command envelope and Bearer adapter; authority is server-owned.
use crate::{accounts::secrets, routes::AppState};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use grove_management_command::{COMMAND_PROTOCOL_VERSION, CommandError, ErrorCode};
use grove_management_policy::Surface;
use grove_management_service::{Proof, resources};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u16,
    command: String,
    input: serde_json::Value,
}

pub(crate) async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Response {
    let hash = token(&headers).map(secrets::token_hash).unwrap_or_default();
    execute_with(state, Proof::Token(&hash), Surface::ResourceApi, body).await
}

pub(crate) async fn execute_with(
    state: AppState,
    proof: Proof<'_>,
    surface: Surface,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Response {
    let request_id = Uuid::new_v4();
    let Ok(Json(value)) = body else {
        return failure(CommandError::rejected(ErrorCode::InvalidInput), request_id);
    };
    if !value.is_object() {
        return failure(CommandError::rejected(ErrorCode::InvalidInput), request_id);
    }
    let Ok(request) = serde_json::from_value::<Request>(value) else {
        return failure(CommandError::rejected(ErrorCode::InvalidInput), request_id);
    };
    let command =
        match grove_management_command::decode(request.protocol, &request.command, request.input) {
            Ok(command) => command,
            Err(error) => return failure(error, request_id),
        };
    let execution = resources::execute(
        &state.pool,
        &state.crypto,
        |input| {
            crate::storage_registration::verify_command(
                &state.crypto,
                state.clock.clone(),
                state.public_url.is_some(),
                input,
            )
        },
        proof,
        surface,
        command,
    )
    .await;
    match execution.result {
        Ok(output) => reply(
            StatusCode::OK,
            serde_json::json!({
                "protocol": COMMAND_PROTOCOL_VERSION, "request_id": execution.request_id,
                "command": output.name().as_str(), "result": output,
            }),
            execution.request_id,
        ),
        Err(error) => failure(error, execution.request_id),
    }
}

pub(crate) fn token(headers: &HeaderMap) -> Option<&str> {
    if headers.contains_key(header::COOKIE)
        || headers.get_all(header::AUTHORIZATION).iter().count() != 1
    {
        return None;
    }
    let (scheme, raw) = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && secrets::valid(raw, secrets::TOKEN_PREFIX))
        .then_some(raw)
}

fn failure(error: CommandError, request_id: Uuid) -> Response {
    let status = match error.code {
        ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Internal | ErrorCode::InvalidResponse => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    reply(
        status,
        serde_json::json!({"protocol": COMMAND_PROTOCOL_VERSION, "request_id": request_id, "error": error}),
        request_id,
    )
}

fn reply(status: StatusCode, body: serde_json::Value, request_id: Uuid) -> Response {
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    if let Ok(value) = request_id.to_string().parse() {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
pub(crate) mod tests;
