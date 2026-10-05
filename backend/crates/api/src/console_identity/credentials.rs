use super::{
    identified,
    inputs::{self, Body, Id, QueryPage},
    output, secrets, session,
};
use crate::routes::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use grove_management_service::{Command, Output};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Issue {
    label: String,
    current_password: filegate_core::SecretString,
    #[serde(default = "default_days")]
    expires_in_days: u16,
}
fn default_days() -> u16 {
    90
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    id: Id,
    query: QueryPage,
) -> Response {
    let (Ok(Path(account)), Ok(Query(query))) = (id, query) else {
        return inputs::invalid();
    };
    let Ok(page) = query.page() else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::Credentials { account, page }).await,
        query.limit,
    )
}
pub(super) async fn issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    id: Id,
    body: Body<Issue>,
) -> Response {
    let (Ok(Path(account)), Ok(Json(body))) = (id, body) else {
        return inputs::invalid();
    };
    if !inputs::valid_label(&body.label) || !(1..=90).contains(&body.expires_in_days) {
        return inputs::invalid();
    }
    let token = secrets::IssuedToken::new();
    let result = session::execute(
        &state,
        &headers,
        Command::IssueCredential {
            account,
            key: token.credential(body.label.trim(), body.expires_in_days, state.clock.now()),
            current_password: body.current_password,
        },
    )
    .await;
    if let Ok(Output::Credential(key)) = &result.result {
        return identified(
            (
                StatusCode::CREATED,
                Json(
                    serde_json::json!({"account_id":key.account_id,"credential_id":key.id,
            "expires_at":key.expires_at,"token":token.expose()}),
                ),
            )
                .into_response(),
            result.request_id,
        );
    }
    output::respond(result, 0)
}
pub(super) async fn revoke(State(state): State<AppState>, headers: HeaderMap, id: Id) -> Response {
    let Ok(Path(id)) = id else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::RevokeCredential(id)).await,
        0,
    )
}
