use std::{path::Path, sync::Arc, time::Duration};

use grove_core::time::Clock;
use grove_db::{PgPool, observation};
use tokio::time::{MissedTickBehavior, interval, timeout};
use tokio_util::sync::CancellationToken;

use crate::spool::{SpoolBudget, spool_root};

const INTERVAL: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) async fn sample(
    request_pool: PgPool,
    maintenance_pool: PgPool,
    spool: Arc<SpoolBudget>,
    clock: Arc<dyn Clock>,
    shutdown: CancellationToken,
) {
    let mut ticker = interval(INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            () = shutdown.cancelled() => return,
            _ = ticker.tick() => {}
        }
        for (name, pool) in [
            ("requests", &request_pool),
            ("maintenance", &maintenance_pool),
        ] {
            metrics::gauge!("grove_db_pool_connections", "pool" => name, "state" => "total")
                .set(f64::from(pool.size()));
            metrics::gauge!("grove_db_pool_connections", "pool" => name, "state" => "idle")
                .set(pool.num_idle() as f64);
            metrics::gauge!("grove_db_pool_connections", "pool" => name, "state" => "limit")
                .set(f64::from(pool.options().get_max_connections()));
        }
        spool.observe();
        let db = timeout(TIMEOUT, observation::snapshot(&maintenance_pool)).await;
        match db {
            Ok(Ok((usage, queues))) => {
                for (state, bytes, files) in [
                    (
                        "active",
                        usage.iter().map(|u| u.active_bytes).sum::<i64>(),
                        usage.iter().map(|u| u.active_files).sum::<i64>(),
                    ),
                    (
                        "reserved",
                        usage.iter().map(|u| u.reserved_bytes).sum::<i64>(),
                        usage.iter().map(|u| u.reserved_files).sum::<i64>(),
                    ),
                    (
                        "purge_pending",
                        usage.iter().map(|u| u.purge_pending_bytes).sum::<i64>(),
                        usage.iter().map(|u| u.purge_pending_files).sum::<i64>(),
                    ),
                ] {
                    metrics::gauge!("grove_objects_bytes", "state" => state).set(bytes as f64);
                    metrics::gauge!("grove_objects", "state" => state).set(files as f64);
                }
                for queue in queues {
                    metrics::gauge!("grove_recovery_queue_unknown_age_objects", "task" => queue.job.clone()).set(queue.unknown_age_objects as f64);
                    // Names come from the fixed VALUES list in the DB observation query.
                    metrics::gauge!("grove_recovery_queue_objects", "task" => queue.job.clone())
                        .set(queue.objects as f64);
                    metrics::gauge!("grove_recovery_queue_oldest_timestamp_seconds", "task" => queue.job).set(
                        queue.oldest_at.map_or(0.0, |time| time.timestamp() as f64));
                }
                snapshot_result("db", true, clock.now().timestamp() as f64);
            }
            _ => snapshot_result("db", false, 0.0),
        }
        match timeout(TIMEOUT, disk_usage(&spool_root())).await {
            Ok(Ok((bytes, files))) => {
                metrics::gauge!("grove_spool_file_bytes").set(bytes as f64);
                metrics::gauge!("grove_spool_files").set(files as f64);
                snapshot_result("spool", true, clock.now().timestamp() as f64);
            }
            _ => snapshot_result("spool", false, 0.0),
        }
    }
}

pub(super) fn snapshot_result(source: &'static str, success: bool, now: f64) {
    metrics::gauge!("grove_snapshot_available", "source" => source).set(if success {
        1.0
    } else {
        0.0
    });
    if success {
        metrics::gauge!("grove_snapshot_last_success_timestamp_seconds", "source" => source)
            .set(now);
    } else {
        metrics::counter!("grove_snapshot_errors_total", "source" => source).increment(1);
    }
}

pub(super) async fn disk_usage(root: &Path) -> std::io::Result<(u64, u64)> {
    let mut directory = tokio::fs::read_dir(root).await?;
    let (mut bytes, mut files) = (0, 0);
    while let Some(entry) = directory.next_entry().await? {
        let kind = match entry.file_type().await {
            Ok(kind) => kind,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if kind.is_file() {
            match entry.metadata().await {
                Ok(metadata) => {
                    bytes += metadata.len();
                    files += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok((bytes, files))
}
