//! Bounded management maintenance, separate from object reconciliation.
use grove_core::ManagementLogRetention;
use grove_db::{
    PgPool,
    management::retention::{self as db, Stream},
};
use std::{num::NonZeroU16, time::Duration};
use tokio::time::Instant;

const BATCH_LIMIT: NonZeroU16 = NonZeroU16::new(1000).expect("positive batch limit");
const JOB_TIMEOUT: Duration = Duration::from_secs(5);

/// The worker holds the management retention lock, not object or identity locks.
pub async fn run(pool: &PgPool, retention: ManagementLogRetention) {
    for (stream, days) in [
        (Stream::Audit, retention.audit_days),
        (Stream::Security, retention.security_days),
        (Stream::Invocations, retention.invocation_days),
    ] {
        let started = Instant::now();
        match tokio::time::timeout(JOB_TIMEOUT, db::prune(pool, stream, days, BATCH_LIMIT)).await {
            Ok(Ok(0)) => tracing::debug!(
                event = "management.logs_pruned",
                stream = stream.name(),
                count = 0,
                retention_days = days.get(),
                duration_ms = started.elapsed().as_millis() as u64,
            ),
            Ok(Ok(count)) => tracing::info!(
                event = "management.logs_pruned",
                stream = stream.name(),
                count,
                retention_days = days.get(),
                duration_ms = started.elapsed().as_millis() as u64,
            ),
            Ok(Err(error)) => tracing::warn!(
                event = "management.retention_failed", stream = stream.name(), %error,
                duration_ms = started.elapsed().as_millis() as u64,
            ),
            Err(_) => tracing::warn!(
                event = "management.retention_failed",
                stream = stream.name(),
                reason = "timeout",
                duration_ms = started.elapsed().as_millis() as u64,
            ),
        }
    }
}
