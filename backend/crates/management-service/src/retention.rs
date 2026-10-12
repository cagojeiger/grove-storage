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
const DAY: NonZeroU16 = NonZeroU16::new(1).expect("positive retention");
const MONTH: NonZeroU16 = NonZeroU16::new(30).expect("positive retention");
const QUARTER: NonZeroU16 = NonZeroU16::new(90).expect("positive retention");
const YEAR: NonZeroU16 = NonZeroU16::new(365).expect("positive retention");

pub struct StreamReport {
    pub stream: Stream,
    pub deleted: u64,
    pub failed: bool,
    pub observed: bool,
    pub oldest_remaining_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Duration,
}

/// The worker holds the management retention lock, not object or identity locks.
pub async fn run(pool: &PgPool, retention: ManagementLogRetention) -> Vec<StreamReport> {
    let mut reports = Vec::new();
    for (stream, days) in [
        (Stream::Audit, retention.audit_days),
        (Stream::Security, retention.security_days),
        (Stream::Invocations, retention.invocation_days),
        (Stream::Sessions, DAY),
        (Stream::PasswordSetup, DAY),
        (Stream::ApiTokens, MONTH),
        (Stream::ObjectAccess, QUARTER),
        (Stream::Usage, YEAR),
    ] {
        let started = Instant::now();
        let deadline = started + JOB_TIMEOUT;
        let mut count = 0;
        let mut oldest_remaining_at = None;
        let mut failed = false;
        let mut observed = false;
        loop {
            match timeout_at(deadline, db::prune(pool, stream, days, BATCH_LIMIT)).await {
                Ok(Ok(batch)) => {
                    observed = true;
                    count += batch.deleted;
                    oldest_remaining_at = batch.oldest_remaining_at;
                    if batch.deleted < u64::from(BATCH_LIMIT.get()) || oldest_remaining_at.is_none()
                    {
                        break;
                    }
                }
                Ok(Err(error)) => {
                    failed = true;
                    tracing::warn!(event = "management.retention_failed", stream = stream.name(), %error);
                    break;
                }
                Err(_) => {
                    failed = true;
                    tracing::warn!(
                        event = "management.retention_failed",
                        stream = stream.name(),
                        reason = "timeout"
                    );
                    break;
                }
            }
        }
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
                ?oldest_remaining_at,
                duration_ms = started.elapsed().as_millis() as u64,
            );
        }
        reports.push(StreamReport {
            stream,
            deleted: count,
            failed,
            observed,
            oldest_remaining_at,
            duration: started.elapsed(),
        });
    }
    reports
}
