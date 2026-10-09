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
        (actor_kind,actor_id,credential_id,session_id,request_id,surface,operation,outcome,error_code,duration_ms)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(f.kind).bind(f.actor).bind(f.credential).bind(f.session)
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
    let write = sqlx::query(
        "INSERT INTO management.security_events
        (actor_kind,actor_id,credential_id,session_id,request_id,surface,event_type,reason_code)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(f.kind)
    .bind(f.actor)
    .bind(f.credential)
    .bind(f.session)
    .bind(request_id)
    .bind(audit::surface_name(surface))
    .bind(event)
    .bind(code);
    if context.is_some() {
        write.execute(pool).await?;
        return Ok(());
    }

    // Anonymous failures are sampled across replicas; no waiting on another logger.
    let mut tx = pool.begin().await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(4674380, hashtext($1))")
            .bind(format!("{}:{code}", audit::surface_name(surface)))
            .fetch_one(&mut *tx)
            .await?;
    if !locked {
        return Ok(());
    }
    let recent: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.security_events
         WHERE created_at > grove_time.wall_now() - interval '1 minute'
           AND actor_kind='anonymous' AND surface=$1 AND reason_code=$2)",
    )
    .bind(audit::surface_name(surface))
    .bind(code)
    .fetch_one(&mut *tx)
    .await?;
    if !recent {
        write.execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
