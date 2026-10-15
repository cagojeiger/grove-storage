//! Bounded history and authentication maintenance, without provider I/O.
use grove_core::ManagementLogRetention;
use grove_db::{
    PgPool,
    retention::{self as db, Stream},
};
use std::{num::NonZeroU16, time::Duration};
use tokio::time::{Instant, timeout_at};

const BATCH_LIMIT: NonZeroU16 = NonZeroU16::new(1000).expect("positive batch limit");
const JOB_TIMEOUT: Duration = Duration::from_secs(2);

pub fn plan(retention: ManagementLogRetention) -> [(Stream, NonZeroU16); 8] {
    const MONTH: NonZeroU16 = NonZeroU16::new(30).expect("positive retention");
    const QUARTER: NonZeroU16 = NonZeroU16::new(90).expect("positive retention");
    const YEAR: NonZeroU16 = NonZeroU16::new(365).expect("positive retention");
    [
        (Stream::Audit, retention.audit_days),
        (Stream::Security, retention.security_days),
        (Stream::Invocations, retention.invocation_days),
        (Stream::Sessions, NonZeroU16::MIN),
        (Stream::PasswordSetup, NonZeroU16::MIN),
        (Stream::ApiTokens, MONTH),
        (Stream::ObjectAccess, QUARTER),
        (Stream::Usage, YEAR),
    ]
}

#[derive(Default)]
pub struct Report {
    pub deleted: u64,
    pub failed: u64,
}

/// The worker holds the management retention lock, not object or identity locks.
pub async fn run(pool: &PgPool, retention: ManagementLogRetention) -> Report {
    let mut report = Report::default();
    for (stream, days) in plan(retention) {
        let started = Instant::now();
        let deadline = started + JOB_TIMEOUT;
        let mut count = 0;
        let mut oldest_remaining_at = None;
        loop {
            match timeout_at(deadline, db::prune(pool, stream, days, BATCH_LIMIT)).await {
                Ok(Ok(batch)) => {
                    count += batch.deleted;
                    oldest_remaining_at = batch.oldest_remaining_at;
                    if batch.deleted < u64::from(BATCH_LIMIT.get()) || oldest_remaining_at.is_none()
                    {
                        break;
                    }
                }
                Ok(Err(error)) => {
                    report.failed += 1;
                    tracing::warn!(event = "management.retention_failed", stream = stream.name(), error_code = "db_prune_failed", %error);
                    break;
                }
                Err(_) => {
                    report.failed += 1;
                    tracing::warn!(
                        event = "management.retention_failed",
                        stream = stream.name(),
                        error_code = "stream_timeout",
                        reason = "timeout"
                    );
                    break;
                }
            }
        }
        report.deleted += count;
        if count == 0 && oldest_remaining_at.is_none() {
            tracing::debug!(
                event = "management.retention_pass",
                stream = stream.name(),
                count
            );
        } else {
            tracing::info!(
                event = "management.retention_pass",
                stream = stream.name(),
                count,
                retention_days = days.get(),
                oldest_remaining_at = oldest_remaining_at
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_default(),
                duration_ms = started.elapsed().as_millis() as u64,
            );
        }
    }
    report
}
