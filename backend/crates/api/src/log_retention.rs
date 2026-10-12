//! Management retention has its own task and lock, without provider I/O.
use grove_core::ManagementLogRetention;
use grove_db::{PgPool, retention};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;

const PASS_TIMEOUT: Duration = Duration::from_secs(20);
// One connection holds the lock, one executes bounded pruning transactions.
pub const DB_CONNECTIONS: u32 = 2;

pub async fn run(
    pool: PgPool,
    policy: ManagementLogRetention,
    tick: Duration,
    shutdown: CancellationToken,
    clock: Arc<dyn grove_core::time::Clock>,
) {
    run_periodic(tick, shutdown, || async {
        let mut observation = crate::metrics::JobObservation::new("management", "retention");
        match tokio::time::timeout(
            PASS_TIMEOUT,
            retention::with_lock(&pool, || {
                grove_management_service::retention::run(&pool, policy)
            }),
        )
        .await
        {
            Ok(Ok(Some(reports))) => {
                let mut total = crate::metrics::JobReport::default();
                let now = clock.now().timestamp() as f64;
                for report in reports {
                    let stream = report.stream.name();
                    let outcome = if report.failed { "error" } else { "success" };
                    total.processed += report.deleted;
                    total.failed += u64::from(report.failed);
                    metrics::counter!("grove_retention_passes_total", "stream" => stream, "outcome" => outcome).increment(1);
                    metrics::counter!("grove_retention_rows_deleted_total", "stream" => stream).increment(report.deleted);
                    metrics::histogram!("grove_retention_duration_seconds", "stream" => stream).record(report.duration.as_secs_f64());
                    if !report.failed {
                        metrics::gauge!("grove_retention_last_success_timestamp_seconds", "stream" => stream).set(now);
                    }
                    if report.observed {
                        metrics::gauge!("grove_retention_oldest_remaining_timestamp_seconds", "stream" => stream)
                            .set(report.oldest_remaining_at.map_or(0.0, |time| time.timestamp() as f64));
                    }
                }
                observation.finish(&total, now);
            }
            Ok(Ok(None)) => observation.skipped(),
            Ok(Err(error)) => {
                observation.finish(&crate::metrics::JobReport { failed: 1, ..Default::default() }, clock.now().timestamp() as f64);
                tracing::warn!(event = "management.maintenance_failed", %error);
            }
            Err(_) => {
                metrics::counter!("grove_worker_timeouts_total", "worker" => "management").increment(1);
                tracing::warn!(event = "management.maintenance_failed", reason = "timeout");
            }
        }
    })
    .await;
}

async fn run_periodic<F: Future<Output = ()>>(
    tick: Duration,
    shutdown: CancellationToken,
    mut job: impl FnMut() -> F,
) {
    let mut ticker = interval(tick);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            () = shutdown.cancelled() => return,
            _ = ticker.tick() => job().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test(start_paused = true)]
    async fn ticks_and_cancellation_use_virtual_time() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let shutdown = CancellationToken::new();
        let stopping = shutdown.clone();
        let worker = tokio::spawn(run_periodic(Duration::from_secs(60), stopping, move || {
            counter.fetch_add(1, Ordering::SeqCst);
            async {}
        }));
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(59)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        shutdown.cancel();
        assert!(worker.await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn already_cancelled_worker_never_runs_a_pass() {
        let shutdown = CancellationToken::new();
        shutdown.cancel();
        let calls = AtomicUsize::new(0);
        run_periodic(Duration::from_secs(60), shutdown, || {
            calls.fetch_add(1, Ordering::SeqCst);
            async {}
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
