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

pub(super) struct ActorColumns {
    pub kind: &'static str,
    pub actor: Option<Uuid>,
    pub owner: Option<Uuid>,
    pub credential: Option<Uuid>,
    pub session: Option<Uuid>,
}

pub(super) fn columns(actor: Option<AuditActor>) -> ActorColumns {
    match actor {
        Some(AuditActor::Master { session_id }) => ActorColumns {
            kind: "master",
            actor: None,
            owner: None,
            credential: None,
            session: session_id,
        },
        Some(AuditActor::User {
            id,
            credential_id,
            session_id,
        }) => ActorColumns {
            kind: "user",
            actor: Some(id),
            owner: None,
            credential: Some(credential_id),
            session: session_id,
        },
        Some(AuditActor::Agent {
            id,
            owner_user_id,
            credential_id,
        }) => ActorColumns {
            kind: "agent",
            actor: Some(id),
            owner: Some(owner_user_id),
            credential: Some(credential_id),
            session: None,
        },
        None => ActorColumns {
            kind: "anonymous",
            actor: None,
            owner: None,
            credential: None,
            session: None,
        },
    }
}

pub(super) fn surface_name(surface: Surface) -> &'static str {
    match surface {
        Surface::Console => "console",
        Surface::Cli => "cli",
        Surface::Mcp => "mcp",
        Surface::ResourceApi => "resource_api",
    }
}

// Metadata is produced by operation-specific SQL, never arbitrary request JSON.
pub(super) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    context: &AuditContext,
    action: &str,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<i64, Error> {
    record_target(tx, context, action, resource_type, &resource_id.to_string()).await
}

pub(super) async fn record_target(
    tx: &mut Transaction<'_, Postgres>,
    context: &AuditContext,
    action: &str,
    resource_type: &str,
    resource_id: &str,
) -> Result<i64, Error> {
    let fields = columns(Some(context.actor));
    let id = sqlx::query_scalar("INSERT INTO management.audit_events
        (actor_kind, actor_id, owner_user_id, credential_id, session_id, request_id, surface, action, resource_type, resource_id)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id")
        .bind(fields.kind).bind(fields.actor).bind(fields.owner).bind(fields.credential).bind(fields.session)
        .bind(context.request_id).bind(surface_name(context.surface)).bind(action).bind(resource_type).bind(resource_id)
        .fetch_one(&mut **tx).await?;
    Ok(id)
}
