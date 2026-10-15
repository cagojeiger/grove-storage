use std::{
    future::Future,
    sync::{Arc, RwLock},
    time::Duration,
};

use grove_core::ManagementLogRetention;
use grove_db::{
    PgPool,
    observations::{self, Snapshot},
};
use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeMetric},
    metrics::gauge::ConstGauge,
};
use tokio::time::{MissedTickBehavior, interval, timeout};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use super::Metrics;
use crate::logging::worker::{Report, Run};

const INTERVAL: Duration = Duration::from_secs(300);
const PASS_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default)]
pub(super) struct Cache(Arc<RwLock<Option<Snapshot>>>);

impl Cache {
    fn replace(&self, snapshot: Snapshot) {
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(snapshot);
    }
}

impl Collector for Cache {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let snapshot = self
            .0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let timestamp = ConstGauge::new(snapshot.as_ref().map_or(0, |s| s.observed_at.timestamp()));
        timestamp.encode(encoder.encode_descriptor(
            "database_snapshot_timestamp_seconds",
            "Database time of the last complete snapshot; zero means none",
            None,
            timestamp.metric_type(),
        )?)?;
        let Some(snapshot) = snapshot.as_ref() else {
            return Ok(());
        };
        gauge(
            &mut encoder,
            "object_backlog_items",
            "Outstanding lifecycle work including retry backoff",
            snapshot
                .objects
                .iter()
                .map(|r| (vec![("queue", r.queue.as_str())], r.items)),
        )?;
        gauge(
            &mut encoder,
            "object_backlog_oldest_waiting_timestamp_seconds",
            "Oldest known queue wait origin; absent when empty or not recorded",
            snapshot.objects.iter().filter_map(|r| {
                r.oldest_waiting_at
                    .map(|at| (vec![("queue", r.queue.as_str())], at.timestamp()))
            }),
        )?;
        gauge(
            &mut encoder,
            "retention_overdue_rows",
            "Rows eligible for existing retention deletion",
            snapshot
                .retention
                .iter()
                .map(|r| (vec![("stream", r.stream)], r.rows)),
        )?;
        gauge(
            &mut encoder,
            "retention_oldest_overdue_timestamp_seconds",
            "Oldest deletion-eligible retention timestamp; zero means empty",
            snapshot.retention.iter().map(|r| {
                (
                    vec![("stream", r.stream)],
                    r.oldest_at.map_or(0, |at| at.timestamp()),
                )
            }),
        )?;
        gauge(
            &mut encoder,
            "retention_days",
            "Configured retention days for each stream",
            snapshot
                .retention
                .iter()
                .map(|r| (vec![("stream", r.stream)], i64::from(r.retention_days))),
        )?;
        gauge(
            &mut encoder,
            "db_table_data_bytes",
            "Allocated table bytes including TOAST, excluding table indexes",
            snapshot.tables.iter().map(|r| {
                (
                    vec![("schema", r.schema.as_str()), ("table", r.table.as_str())],
                    r.data_bytes,
                )
            }),
        )?;
        gauge(
            &mut encoder,
            "db_table_index_bytes",
            "Allocated table index bytes",
            snapshot.tables.iter().map(|r| {
                (
                    vec![("schema", r.schema.as_str()), ("table", r.table.as_str())],
                    r.index_bytes,
                )
            }),
        )?;
        gauge(
            &mut encoder,
            "db_table_estimated_rows",
            "PostgreSQL live and dead row estimates, not exact counts",
            snapshot.tables.iter().flat_map(|r| {
                [
                    (
                        vec![
                            ("schema", r.schema.as_str()),
                            ("table", r.table.as_str()),
                            ("state", "live"),
                        ],
                        r.estimated_live_rows,
                    ),
                    (
                        vec![
                            ("schema", r.schema.as_str()),
                            ("table", r.table.as_str()),
                            ("state", "dead"),
                        ],
                        r.estimated_dead_rows,
                    ),
                ]
            }),
        )?;
        Ok(())
    }
}

fn gauge<'a>(
    encoder: &mut DescriptorEncoder,
    name: &str,
    help: &str,
    samples: impl Iterator<Item = (Vec<(&'static str, &'a str)>, i64)>,
) -> Result<(), std::fmt::Error> {
    let mut family =
        encoder.encode_descriptor(name, help, None, ConstGauge::new(0).metric_type())?;
    for (labels, value) in samples {
        ConstGauge::new(value).encode(family.encode_family(&labels)?)?;
    }
    Ok(())
}

pub(crate) async fn run(
    pool: PgPool,
    policy: ManagementLogRetention,
    metrics: Metrics,
    shutdown: CancellationToken,
) {
    metrics.register_worker(&super::WorkerLabels {
        worker: "database",
        task: "snapshot",
    });
    periodic(INTERVAL, shutdown, move || {
        let pool = pool.clone();
        let metrics = metrics.clone();
        async move { refresh(&pool, policy, &metrics).await }
    })
    .await;
}

pub(crate) async fn refresh(pool: &PgPool, policy: ManagementLogRetention, metrics: &Metrics) {
    let mut run = Run::new("database", "snapshot", metrics.clone());
    let plan = grove_management_service::retention::plan(policy);
    match timeout(PASS_TIMEOUT, observations::collect(pool, plan))
        .instrument(run.span.clone())
        .await
    {
        Ok(Ok(snapshot)) => {
            metrics.0.database.replace(snapshot);
            run.finish(&Report::default());
        }
        Ok(Err(error)) => {
            // Keep the last complete snapshot; do not turn collection failures into zeroes.
            tracing::warn!(parent: &run.span, event = "database.snapshot_failed", error_code = "db_snapshot_failed",
                stage = error.stage, stream = error.stream.unwrap_or("none"), reason = error.reason);
            run.finish(&Report {
                failed: 1,
                ..Report::default()
            });
        }
        Err(_) => {
            tracing::warn!(parent: &run.span, event = "database.snapshot_failed", error_code = "db_snapshot_failed",
                stage = "collection", stream = "none", reason = "pass_timeout");
            run.timed_out();
        }
    }
}

async fn periodic<F: Future<Output = ()>>(
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
            _ = ticker.tick() => {
                tokio::select! {
                    biased;
                    () = shutdown.cancelled() => return,
                    () = job() => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
