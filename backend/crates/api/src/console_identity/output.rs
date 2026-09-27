//! Explicit HTTP allowlists keep verification material out of list responses.
use super::{failure, identified};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use filegate_db::management::history::EventContext;
use grove_management_service::{Error, Execution, Output};
use serde_json::{Value, json};

pub(super) fn respond(execution: Execution, limit: u16) -> Response {
    let result = match execution.result {
        Err(error) => return failure(error, execution.request_id),
        Ok(output) => output,
    };
    let body = match result {
        Output::Account(id) => {
            return identified(
                (StatusCode::CREATED, Json(json!({"account_id": id}))).into_response(),
                execution.request_id,
            );
        }
        Output::Changed(changed) => json!({"changed":changed}),
        Output::AccountDetails(row) => account(row),
        Output::Accounts(page) => json!({
            "items": page.items.into_iter().map(account).collect::<Vec<_>>(),
            "next_before": page.next_before,
            "previous_after": page.previous_after,
            "initialized": page.initialized,
        }),
        Output::Credentials(rows) => credential_page(rows, limit),
        Output::Sessions(rows) => page(rows.into_iter().map(|r| {
            (r.id.to_string(), json!({
                "id": r.id, "credential_id": r.credential_id, "created_at": r.created_at,
                "expires_at": r.expires_at, "revoked_at": r.revoked_at,
            }))
        }), limit),
        Output::Audit(rows) => page(rows.into_iter().map(|r| {
            (r.context.id.to_string(), json!({
                "context": context(r.context), "action": r.action,
                "resource_type": r.resource_type, "resource_id": r.resource_id,
                "metadata": r.metadata,
            }))
        }), limit),
        Output::Invocations(rows) => page(rows.into_iter().map(|r| {
            (r.context.id.to_string(), json!({
                "context": context(r.context), "operation": r.operation,
                "outcome": r.outcome, "error_code": r.error_code, "duration_ms": r.duration_ms,
            }))
        }), limit),
        Output::Security(rows) => page(rows.into_iter().map(|r| {
            (r.context.id.to_string(), json!({
                "context": context(r.context), "event_type": r.event_type,
                "reason_code": r.reason_code,
            }))
        }), limit),
        _ => return failure(Error::Unavailable, execution.request_id),
    };
    identified(Json(body).into_response(), execution.request_id)
}

pub(super) fn credential_page(
    rows: Vec<filegate_db::management::queries::CredentialSummary>,
    limit: u16,
) -> Value {
    page(
        rows.into_iter().map(|r| {
            (
                r.id.to_string(),
                json!({
                    "id": r.id, "account_id": r.account_id, "label": r.label,
                    "token_prefix": r.token_prefix, "created_at": r.created_at,
                    "expires_at": r.expires_at, "revoked_at": r.revoked_at,
                }),
            )
        }),
        limit,
    )
}

fn account(r: filegate_db::management::queries::AccountSummary) -> Value {
    json!({
        "id": r.id, "kind": r.kind, "display_name": r.display_name,
        "role": r.role, "is_active": r.is_active, "deleted_at": r.deleted_at,
        "username": r.login_name, "password_ready": r.password_ready,
    })
}

fn page(rows: impl Iterator<Item = (String, Value)>, limit: u16) -> Value {
    let (ids, items): (Vec<_>, Vec<_>) = rows.unzip();
    let next_before = if items.len() == usize::from(limit) {
        ids.last()
    } else {
        None
    };
    json!({"items":items,"next_before":next_before})
}
fn context(c: EventContext) -> Value {
    json!({
        "id": c.id.to_string(), "created_at": c.created_at, "actor_kind": c.actor_kind,
        "actor_id": c.actor_id, "owner_user_id": c.owner_user_id,
        "credential_id": c.credential_id, "session_id": c.session_id,
        "request_id": c.request_id, "surface": c.surface,
    })
}
