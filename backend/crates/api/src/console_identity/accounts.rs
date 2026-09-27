use super::{
    inputs::{self, Body, Id, Role},
    output, session,
};
use crate::routes::AppState;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::HeaderMap,
    response::Response,
};
use filegate_db::management::queries::{AccountQuery, AccountStatus, Page};
use filegate_db::management::{AccountChange, NewAccount};
use grove_management_service::Command;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    #[default]
    All,
    Current,
    Active,
    Disabled,
    Deleted,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct List {
    before: Option<Uuid>,
    after: Option<Uuid>,
    #[serde(default = "list_limit")]
    limit: u16,
    #[serde(default)]
    q: String,
    role: Option<Role>,
    #[serde(default)]
    status: Status,
}
fn list_limit() -> u16 {
    50
}
impl List {
    fn query(self) -> Result<AccountQuery, filegate_db::management::Error> {
        AccountQuery::new(
            Page::new(self.before, self.limit)?,
            self.after,
            self.q,
            self.role.map(Into::into),
            match self.status {
                Status::All => AccountStatus::All,
                Status::Current => AccountStatus::Current,
                Status::Active => AccountStatus::Active,
                Status::Disabled => AccountStatus::Disabled,
                Status::Deleted => AccountStatus::Deleted,
            },
        )
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Create {
    User { display_name: String, role: Role },
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
    query: Result<Query<List>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return inputs::invalid();
    };
    let limit = query.limit;
    let Ok(page) = query.query() else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::Accounts(page)).await,
        limit,
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
        Create::User { display_name, role } if inputs::valid_label(display_name) => NewAccount {
            display_name: display_name.trim(),
            role: (*role).into(),
        },
        _ => return inputs::invalid(),
    };
    output::respond(
        session::execute(&state, &headers, Command::CreateAccount(account)).await,
        0,
    )
}
pub(super) async fn get(State(state): State<AppState>, headers: HeaderMap, id: Id) -> Response {
    let Ok(Path(id)) = id else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::Account(id)).await,
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
