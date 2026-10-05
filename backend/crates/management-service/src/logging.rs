use crate::Error;
use filegate_db::{
    PgPool,
    management::{
        self as db, AuditActor, AuditContext,
        telemetry::{self, Outcome, SecurityReason},
    },
};
use grove_management_policy::Surface;
use std::{future::Future, time::Duration};
use uuid::Uuid;

const LOG_TIMEOUT: Duration = Duration::from_millis(250);

pub(crate) async fn session_operation<T>(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    operation: &'static str,
    work: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let started = tokio::time::Instant::now();
    let context = db::session_actor(pool, session_hash)
        .await
        .ok()
        .flatten()
        .map(|actor| AuditContext {
            actor: AuditActor::User {
                id: actor.account_id,
                credential_id: actor.credential_id,
                session_id: actor.session_id,
            },
            request_id,
            surface: Surface::Console,
        });
    let result = work.await;
    record(
        pool,
        context.as_ref(),
        request_id,
        Surface::Console,
        operation,
        started.elapsed(),
        &result,
    )
    .await;
    result
}

pub(super) async fn record<T>(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    operation: &'static str,
    elapsed: Duration,
    result: &Result<T, Error>,
) {
    if let Some(context) = context {
        let (outcome, code) = match result {
            Ok(_) => (Outcome::Succeeded, None),
            Err(Error::Forbidden) => (Outcome::Denied, Some(Error::Forbidden.code())),
            Err(Error::OutcomeUnknown) => (Outcome::Unknown, Some(Error::OutcomeUnknown.code())),
            Err(error) => (Outcome::Failed, Some(error.code())),
        };
        let write = telemetry::invocation(
            pool,
            context,
            operation,
            outcome,
            code,
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        );
        if !matches!(tokio::time::timeout(LOG_TIMEOUT, write).await, Ok(Ok(()))) {
            tracing::warn!(%request_id, stream="command_invocations", "Management history write failed or timed out");
        }
    }
    let reason = match result {
        Err(Error::Unauthenticated) => Some(SecurityReason::Unauthenticated),
        Err(Error::Forbidden) => Some(SecurityReason::Forbidden),
        Err(Error::RateLimited) => Some(SecurityReason::RateLimited),
        Err(_) if context.is_none() => Some(SecurityReason::Unavailable),
        _ => None,
    };
    if let Some(reason) = reason {
        security(pool, context, request_id, surface, reason).await;
    }
}

pub(super) async fn security(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    reason: SecurityReason,
) {
    let write = telemetry::security(pool, context, request_id, surface, reason);
    if !matches!(tokio::time::timeout(LOG_TIMEOUT, write).await, Ok(Ok(()))) {
        tracing::warn!(%request_id, stream="security_events", "Management history write failed or timed out");
    }
}
