use super::{
    inputs::{self, AgentRole, Body, Id, QueryPage, Role},
    output, session,
};
use crate::routes::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
};
use filegate_db::management::{AccountChange, NewAccount};
use grove_management_service::Command;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Create {
    User {
        display_name: String,
        role: Role,
    },
    Agent {
        display_name: String,
        role: AgentRole,
        owner_user_id: Uuid,
    },
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Change {
    Role { role: Role },
    Active { is_active: bool },
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: QueryPage,
) -> Response {
    let Ok(Query(query)) = query else {
        return inputs::invalid();
    };
    let Ok(page) = query.page() else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::Accounts(page)).await,
        query.limit,
    )
}
pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<Create>,
) -> Response {
    let Ok(Json(body)) = body else {
        return inputs::invalid();
    };
    let account = match &body {
        Create::User { display_name, role } if inputs::valid_label(display_name) => {
            NewAccount::User {
                display_name: display_name.trim(),
                role: (*role).into(),
            }
        }
        Create::Agent {
            display_name,
            role,
            owner_user_id,
        } if inputs::valid_label(display_name) => NewAccount::Agent {
            display_name: display_name.trim(),
            role: (*role).into(),
            owner_user_id: *owner_user_id,
        },
        _ => return inputs::invalid(),
    };
    output::respond(
        session::execute(&state, &headers, Command::CreateAccount(account)).await,
        0,
    )
}
pub(super) async fn change(
    State(state): State<AppState>,
    headers: HeaderMap,
    id: Id,
    body: Body<Change>,
) -> Response {
    let (Ok(Path(id)), Ok(Json(body))) = (id, body) else {
        return inputs::invalid();
    };
    let change = match body {
        Change::Role { role } => AccountChange::Role(role.into()),
        Change::Active { is_active } => AccountChange::Active(is_active),
    };
    output::respond(
        session::execute(&state, &headers, Command::ChangeAccount { id, change }).await,
        0,
    )
}
pub(super) async fn remove(State(state): State<AppState>, headers: HeaderMap, id: Id) -> Response {
    let Ok(Path(id)) = id else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(
            &state,
            &headers,
            Command::ChangeAccount {
                id,
                change: AccountChange::Delete,
            },
        )
        .await,
        0,
    )
}
