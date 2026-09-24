use super::{
    inputs::{self, Id, QueryPage},
    output, session,
};
use crate::routes::AppState;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
};
use grove_management_service::Command;

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
    query: QueryPage,
) -> Response {
    list(state, headers, query, Stream::Audit).await
}
pub(super) async fn invocations(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: QueryPage,
) -> Response {
    list(state, headers, query, Stream::Invocations).await
}
pub(super) async fn security(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: QueryPage,
) -> Response {
    list(state, headers, query, Stream::Security).await
}
async fn list(state: AppState, headers: HeaderMap, query: QueryPage, stream: Stream) -> Response {
    let Ok(Query(query)) = query else {
        return inputs::invalid();
    };
    let Ok(page) = query.history_page() else {
        return inputs::invalid();
    };
    let command = match stream {
        Stream::Audit => Command::Audit(page),
        Stream::Invocations => Command::Invocations(page),
        Stream::Security => Command::Security(page),
    };
    output::respond(
        session::execute(&state, &headers, command).await,
        query.limit,
    )
}
