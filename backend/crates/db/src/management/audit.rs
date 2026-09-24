use grove_management_policy::Surface;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::Error;

#[derive(Clone, Copy)]
pub enum AuditActor {
    Master {
        session_id: Option<Uuid>,
    },
    User {
        id: Uuid,
        credential_id: Uuid,
        session_id: Option<Uuid>,
    },
    Agent {
        id: Uuid,
        owner_user_id: Uuid,
        credential_id: Uuid,
    },
}

#[derive(Clone, Copy)]
pub struct AuditContext {
    pub actor: AuditActor,
    pub request_id: Uuid,
    pub surface: Surface,
}

// Metadata is produced by operation-specific SQL, never arbitrary request JSON.
pub(super) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    context: &AuditContext,
    action: &str,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<i64, Error> {
    let (kind, actor, owner, credential, session) = match context.actor {
        AuditActor::Master { session_id } => ("master", None, None, None, session_id),
        AuditActor::User {
            id,
            credential_id,
            session_id,
        } => ("user", Some(id), None, Some(credential_id), session_id),
        AuditActor::Agent {
            id,
            owner_user_id,
            credential_id,
        } => (
            "agent",
            Some(id),
            Some(owner_user_id),
            Some(credential_id),
            None,
        ),
    };
    let surface = match context.surface {
        Surface::Console => "console",
        Surface::Cli => "cli",
        Surface::Mcp => "mcp",
        Surface::ResourceApi => "resource_api",
    };
    let id = sqlx::query_scalar("INSERT INTO management.audit_events
        (actor_kind, actor_id, owner_user_id, credential_id, session_id, request_id, surface, action, resource_type, resource_id)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id")
        .bind(kind).bind(actor).bind(owner).bind(credential).bind(session)
        .bind(context.request_id).bind(surface).bind(action).bind(resource_type).bind(resource_id.to_string())
        .fetch_one(&mut **tx).await?;
    Ok(id)
}
