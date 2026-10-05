//! Best-effort streams: callers bound latency and report failures separately.
use super::{AuditContext, Error, audit};
use grove_management_policy::Surface;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum Outcome {
    Succeeded,
    Denied,
    Failed,
    Unknown,
}
impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy)]
pub enum SecurityReason {
    Authenticated,
    Unauthenticated,
    Forbidden,
    Unavailable,
    RateLimited,
}
impl SecurityReason {
    fn fields(self) -> (&'static str, &'static str) {
        match self {
            Self::Authenticated => ("authentication_succeeded", "authenticated"),
            Self::Unauthenticated => ("authentication_failed", "unauthenticated"),
            Self::Forbidden => ("permission_denied", "forbidden"),
            Self::Unavailable => ("authentication_unavailable", "unavailable"),
            Self::RateLimited => ("authentication_rate_limited", "rate_limited"),
        }
    }
}

pub async fn invocation(
    pool: &PgPool,
    context: &AuditContext,
    operation: &'static str,
    outcome: Outcome,
    error_code: Option<&'static str>,
    duration_ms: u64,
) -> Result<(), Error> {
    let f = audit::columns(Some(context.actor));
    sqlx::query("INSERT INTO management.command_invocations
        (actor_kind,actor_id,owner_user_id,credential_id,session_id,request_id,surface,operation,outcome,error_code,duration_ms)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(f.kind).bind(f.actor).bind(f.owner).bind(f.credential).bind(f.session)
        .bind(context.request_id).bind(audit::surface_name(context.surface)).bind(operation).bind(outcome.name()).bind(error_code)
        .bind(i64::try_from(duration_ms).unwrap_or(i64::MAX)).execute(pool).await?;
    Ok(())
}

pub async fn security(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    reason: SecurityReason,
) -> Result<(), Error> {
    let f = audit::columns(context.map(|c| c.actor));
    let (event, code) = reason.fields();
    sqlx::query("INSERT INTO management.security_events
        (actor_kind,actor_id,owner_user_id,credential_id,session_id,request_id,surface,event_type,reason_code)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(f.kind).bind(f.actor).bind(f.owner).bind(f.credential).bind(f.session)
        .bind(request_id).bind(audit::surface_name(surface)).bind(event).bind(code).execute(pool).await?;
    Ok(())
}
