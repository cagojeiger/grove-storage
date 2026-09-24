use super::{browser, failure, identified, secrets};
use crate::routes::AppState;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use filegate_core::{ExposeSecret, SecretString};
use grove_management_service::{
    Error,
    master::{self as service, Command, Output},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Login {
    token: SecretString,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bootstrap {
    display_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recover {
    user_id: Uuid,
    confirm: bool,
}

pub(super) async fn login(
    State(state): State<AppState>,
    body: Result<Json<Login>, JsonRejection>,
) -> Response {
    let Some(config) = state.master.as_deref() else {
        return failure(Error::NotFound, Uuid::new_v4());
    };
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, Uuid::new_v4());
    };
    let hash = secrets::valid(body.token.expose_secret(), secrets::MASTER_PREFIX)
        .then(|| secrets::master_hash(body.token.expose_secret()));
    let raw = SecretString::from(format!(
        "{}{}",
        secrets::MASTER_SESSION_PREFIX,
        filegate_core::generate_url_secret()
    ));
    let execution = service::login(
        &state.pool,
        config,
        hash.as_deref(),
        &secrets::master_session_hash(raw.expose_secret()),
    )
    .await;
    match execution.result {
        Ok(session) => {
            let mut response = Json(serde_json::json!({"principal":"master","scope":"setup_recovery","session_id":session.id,"expires_at":session.expires_at})).into_response();
            browser::set_master_cookie(
                &mut response,
                raw.expose_secret(),
                (session.expires_at - chrono::Utc::now())
                    .num_seconds()
                    .max(0),
            );
            identified(response, execution.request_id)
        }
        Err(error) => failure(error, execution.request_id),
    }
}

pub(super) async fn current(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let execution = execute(&state, &headers, Command::Current).await;
    match execution.result {
        Ok(Output::Current {
            session,
            initialized,
        }) => identified(
            Json(
                serde_json::json!({"principal":"master","scope":"setup_recovery",
            "session_id":session.id,"expires_at":session.expires_at,"initialized":initialized}),
            )
            .into_response(),
            execution.request_id,
        ),
        Err(error) => failure(error, execution.request_id),
        _ => failure(Error::Unavailable, execution.request_id),
    }
}

pub(super) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let execution = execute(&state, &headers, Command::Logout).await;
    let mut response = match execution.result {
        Ok(Output::LoggedOut) => {
            identified(StatusCode::NO_CONTENT.into_response(), execution.request_id)
        }
        Err(Error::Unauthenticated) => failure(Error::Unauthenticated, execution.request_id),
        Err(error) => return failure(error, execution.request_id),
        _ => return failure(Error::Unavailable, execution.request_id),
    };
    browser::set_master_cookie(&mut response, "", 0);
    response
}

pub(super) async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<Bootstrap>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, Uuid::new_v4());
    };
    let name = body.display_name.trim();
    if !(1..=80).contains(&name.chars().count()) {
        return failure(Error::InvalidInput, Uuid::new_v4());
    }
    issue(&state, &headers, Change::Bootstrap(name)).await
}

pub(super) async fn recover(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<Recover>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return failure(Error::InvalidInput, Uuid::new_v4());
    };
    if !body.confirm {
        return failure(Error::InvalidInput, Uuid::new_v4());
    }
    issue(&state, &headers, Change::Recover(body.user_id)).await
}

enum Change<'a> {
    Bootstrap(&'a str),
    Recover(Uuid),
}
async fn issue(state: &AppState, headers: &HeaderMap, change: Change<'_>) -> Response {
    let token = secrets::IssuedToken::new();
    let key = token.credential("master-issued", 90);
    let command = match change {
        Change::Bootstrap(name) => Command::Bootstrap { name, key },
        Change::Recover(account) => Command::Recover { account, key },
    };
    let execution = execute(state, headers, command).await;
    match execution.result {
        Ok(Output::Credential(key)) => {
            let mut response = (
                StatusCode::CREATED,
                Json(
                    serde_json::json!({"user_id":key.account_id,"credential_id":key.id,
                "expires_at":key.expires_at,"token":token.expose()}),
                ),
            )
                .into_response();
            browser::set_master_cookie(&mut response, "", 0);
            identified(response, execution.request_id)
        }
        Err(error) => failure(error, execution.request_id),
        _ => failure(Error::Unavailable, execution.request_id),
    }
}

async fn execute(
    state: &AppState,
    headers: &HeaderMap,
    command: Command<'_>,
) -> service::Execution {
    let Some(config) = state.master.as_deref() else {
        return service::Execution {
            request_id: Uuid::new_v4(),
            result: Err(Error::NotFound),
        };
    };
    let hash = browser::master_cookie(headers)
        .map(secrets::master_session_hash)
        .unwrap_or_default();
    service::execute(&state.pool, config, &hash, command).await
}
