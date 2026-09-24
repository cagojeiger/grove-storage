use crate::{Error, Output};
use filegate_db::{
    PgPool,
    management::{
        AuditContext,
        telemetry::{self, Outcome, SecurityReason},
    },
};
use grove_management_policy::Surface;
use std::time::Duration;
use uuid::Uuid;

const LOG_TIMEOUT: Duration = Duration::from_millis(250);

pub(super) async fn record(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    operation: &'static str,
    elapsed: Duration,
    result: &Result<Output, Error>,
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
        Err(_) if context.is_none() => Some(SecurityReason::Unavailable),
        _ => None,
    };
    if let Some(reason) = reason {
        let write = telemetry::security(pool, context, request_id, surface, reason);
        if !matches!(tokio::time::timeout(LOG_TIMEOUT, write).await, Ok(Ok(()))) {
            tracing::warn!(%request_id, stream="security_events", "Management history write failed or timed out");
        }
    }
}
