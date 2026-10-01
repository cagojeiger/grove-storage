use super::{
    inputs::{self, Id, Pagination, QueryPage},
    output, session,
};
use crate::routes::AppState;
use axum::{
    extract::{Path, Query, State, rejection::QueryRejection},
    http::HeaderMap,
    response::Response,
};
use filegate_db::management::history::HistoryQuery;
use grove_management_service::Command;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HistoryParams {
    before: Option<String>,
    #[serde(default = "history_limit")]
    limit: u16,
    account_id: Option<Uuid>,
    credential_id: Option<Uuid>,
}
fn history_limit() -> u16 {
    50
}
type HistoryInput = Result<Query<HistoryParams>, QueryRejection>;

pub(super) async fn sessions(
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
        session::execute(&state, &headers, Command::OwnSessions(page)).await,
        query.limit,
    )
}
pub(super) async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    id: Id,
) -> Response {
    let Ok(Path(id)) = id else {
        return inputs::invalid();
    };
    output::respond(
        session::execute(&state, &headers, Command::RevokeOwnSession(id)).await,
        0,
    )
}

enum Stream {
    Audit,
    Invocations,
    Security,
}
pub(super) async fn audit(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: HistoryInput,
) -> Response {
    list(state, headers, query, Stream::Audit).await
}
pub(super) async fn invocations(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: HistoryInput,
) -> Response {
    list(state, headers, query, Stream::Invocations).await
}
pub(super) async fn security(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: HistoryInput,
) -> Response {
    list(state, headers, query, Stream::Security).await
}
async fn list(
    state: AppState,
    headers: HeaderMap,
    query: HistoryInput,
    stream: Stream,
) -> Response {
    let Ok(Query(query)) = query else {
        return inputs::invalid();
    };
    let Ok(page) = (Pagination {
        before: query.before,
        limit: query.limit,
    })
    .history_page() else {
        return inputs::invalid();
    };
    let history = HistoryQuery {
        page,
        account_id: query.account_id,
        credential_id: query.credential_id,
    };
    let command = match stream {
        Stream::Audit => Command::Audit(history),
        Stream::Invocations => Command::Invocations(history),
        Stream::Security => Command::Security(history),
    };
    output::respond(
        session::execute(&state, &headers, command).await,
        query.limit,
    )
}
