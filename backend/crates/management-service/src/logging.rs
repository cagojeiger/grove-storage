use crate::Error;
use grove_db::{
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
            actor: AuditActor::Account {
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
    observe(surface, operation, elapsed, result);
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
            metrics::counter!("grove_history_write_errors_total", "stream" => "invocations")
                .increment(1);
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

fn observe<T>(
    surface: Surface,
    operation: &'static str,
    elapsed: Duration,
    result: &Result<T, Error>,
) {
    let surface = match surface {
        Surface::Console => "console",
        Surface::Cli => "cli",
        Surface::Mcp => "mcp",
        Surface::ResourceApi => "resource_api",
    };
    let outcome = match result {
        Ok(_) => "success",
        Err(Error::Forbidden) => "denied",
        Err(Error::OutcomeUnknown) => "unknown",
        Err(_) => "error",
    };
    metrics::counter!("grove_management_operations_total", "surface" => surface,
        "operation" => operation, "outcome" => outcome)
    .increment(1);
    metrics::histogram!("grove_management_duration_seconds", "surface" => surface,
        "operation" => operation)
    .record(elapsed.as_secs_f64());
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
        metrics::counter!("grove_history_write_errors_total", "stream" => "security").increment(1);
        tracing::warn!(%request_id, stream="security_events", "Management history write failed or timed out");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn command_outcomes_are_independent_of_transport_status_and_identity() {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            for result in [
                Ok(()),
                Err(Error::Forbidden),
                Err(Error::OutcomeUnknown),
                Err(Error::Unauthenticated),
            ] {
                observe(
                    Surface::Mcp,
                    "storage.list",
                    Duration::from_secs(2),
                    &result,
                );
            }
        });
        let text = handle.render();
        for outcome in ["success", "denied", "unknown", "error"] {
            assert!(text.contains(&format!(
                "grove_management_operations_total{{surface=\"mcp\",operation=\"storage.list\",outcome=\"{outcome}\"}} 1"
            )), "{text}");
        }
        assert!(!text.contains("account_id"));
    }
}
