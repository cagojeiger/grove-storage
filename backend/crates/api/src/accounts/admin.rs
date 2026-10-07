use super::{
    browser, failure, identified,
    inputs::{self, Body, Id, Role},
    output, secrets, session,
};
use crate::routes::AppState;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use grove_core::{ExposeSecret, SecretString};
use grove_db::management::queries::{AccountQuery, AccountStatus, Page};
use grove_db::management::{AccountChange, NewAccount};
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
    fn query(self) -> Result<AccountQuery, grove_db::management::Error> {
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
    User {
        display_name: String,
        role: Role,
    },
    UserWithPasswordSetup {
        display_name: String,
        role: Role,
        username: String,
        current_password: SecretString,
    },
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Change {
    Name { display_name: String },
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
    match body {
        Create::User { display_name, role } if inputs::valid_label(&display_name) => {
            output::respond(
                session::execute(
                    &state,
                    &headers,
                    Command::CreateAccount(NewAccount {
                        display_name: display_name.trim(),
                        role: role.into(),
                    }),
                )
                .await,
                0,
            )
        }
        Create::UserWithPasswordSetup {
            display_name,
            role,
            username,
            current_password,
        } if inputs::valid_label(&display_name) => {
            let request_id = Uuid::new_v4();
            let session_hash = browser::session_hash(&headers);
            if browser::cookie(&headers).is_none() {
                return failure(grove_management_service::Error::Unauthenticated, request_id);
            }
            let token = SecretString::from(format!(
                "{}{}",
                secrets::SETUP_PREFIX,
                grove_core::generate_url_secret()
            ));
            match grove_management_service::password_setups::create(
                &state.pool,
                request_id,
                &session_hash,
                NewAccount {
                    display_name: display_name.trim(),
                    role: role.into(),
                },
                &username,
                current_password,
                &secrets::setup_hash(token.expose_secret()),
            )
            .await
            {
                Ok(setup) => identified(
                    (
                        StatusCode::CREATED,
                        Json(serde_json::json!({
                            "account_id": setup.account_id,
                            "username": setup.login_name,
                            "expires_at": setup.expires_at,
                            "token": token.expose_secret(),
                        })),
                    )
                        .into_response(),
                    request_id,
                ),
                Err(error) => failure(error, request_id),
            }
        }
        _ => inputs::invalid(),
    }
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
        Change::Name { display_name } => {
            if !inputs::valid_label(&display_name) {
                return inputs::invalid();
            }
            AccountChange::Name(display_name)
        }
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
