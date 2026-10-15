#![allow(clippy::unwrap_used)]
use super::*;
use grove_db::retention;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

fn policy() -> ManagementLogRetention {
    ManagementLogRetention {
        audit_days: std::num::NonZeroU16::new(365).unwrap(),
        security_days: std::num::NonZeroU16::new(90).unwrap(),
        invocation_days: std::num::NonZeroU16::new(90).unwrap(),
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        observed_at: chrono::DateTime::from_timestamp(1000, 0).unwrap(),
        objects: vec![observations::ObjectBacklog {
            queue: "purge".into(),
            items: 3,
            oldest_waiting_at: chrono::DateTime::from_timestamp(500, 0),
        }],
        retention: vec![retention::Backlog {
            stream: "audit_events",
            retention_days: 365,
            rows: 2,
            oldest_at: chrono::DateTime::from_timestamp(100, 0),
        }],
        tables: vec![observations::Table {
            schema: "public".into(),
            table: "files".into(),
            data_bytes: 8192,
            index_bytes: 16384,
            estimated_live_rows: 8,
            estimated_dead_rows: 2,
        }],
    }
}

#[tokio::test]
async fn snapshot_is_absent_until_collected_and_failure_preserves_previous_values() {
    let metrics = Metrics::default();
    let text = metrics.render().unwrap();
    assert!(text.contains("grove_database_snapshot_timestamp_seconds 0"));
    assert!(!text.contains("grove_object_backlog_items{"));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let policy = policy();
    refresh(&pool, policy, &metrics).await;
    assert!(
        !metrics
            .render()
            .unwrap()
            .contains("grove_object_backlog_items{")
    );
    metrics.0.database.replace(snapshot());
    let text = metrics.render().unwrap();
    assert!(text.contains("grove_database_snapshot_timestamp_seconds 1000"));
    assert!(text.contains("grove_object_backlog_items{queue=\"purge\"} 3"));
    assert!(
        text.contains("grove_object_backlog_oldest_waiting_timestamp_seconds{queue=\"purge\"} 500")
    );
    assert!(text.contains("grove_retention_days{stream=\"audit_events\"} 365"));
    assert!(text.contains(
        "grove_db_table_estimated_rows{schema=\"public\",table=\"files\",state=\"dead\"} 2"
    ));
    refresh(&pool, policy, &metrics).await;
    assert!(
        metrics
            .render()
            .unwrap()
            .contains("grove_object_backlog_items{queue=\"purge\"} 3")
    );
    assert!(
        metrics
            .render()
            .unwrap()
            .contains("grove_database_snapshot_timestamp_seconds 1000")
    );
    metrics.0.database.replace(Snapshot {
        objects: vec![observations::ObjectBacklog {
            queue: "purge".into(),
            items: 0,
            oldest_waiting_at: None,
        }],
        ..snapshot()
    });
    assert!(
        metrics
            .render()
            .unwrap()
            .contains("grove_object_backlog_items{queue=\"purge\"} 0")
    );
    assert!(
        !metrics
            .render()
            .unwrap()
            .contains("grove_object_backlog_oldest_waiting_timestamp_seconds{queue=\"purge\"}")
    );
}

#[tokio::test(start_paused = true)]
async fn snapshot_cadence_and_cancellation_use_virtual_time() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let shutdown = CancellationToken::new();
    let stopping = shutdown.clone();
    let task = tokio::spawn(periodic(INTERVAL, stopping, move || {
        counter.fetch_add(1, Ordering::SeqCst);
        async {}
    }));
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(INTERVAL - Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    shutdown.cancel();
    task.await.unwrap();
    periodic(INTERVAL, shutdown, || async {
        calls.fetch_add(1, Ordering::SeqCst);
    })
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn shutdown_interrupts_a_stalled_collection() {
    let metrics = Metrics::default();
    let shared = metrics.clone();
    let shutdown = CancellationToken::new();
    let stopping = shutdown.clone();
    let task = tokio::spawn(periodic(INTERVAL, stopping, move || {
        let metrics = shared.clone();
        async move {
            let _run = Run::new("database", "snapshot", metrics);
            std::future::pending::<()>().await;
        }
    }));
    tokio::task::yield_now().await;
    shutdown.cancel();
    task.await.unwrap();
    let text = metrics.render().unwrap();
    assert!(text.contains(
        "grove_worker_runs_total{worker=\"database\",task=\"snapshot\",outcome=\"interrupted\"} 1"
    ));
    assert!(text.contains("grove_worker_runs_in_flight{worker=\"database\",task=\"snapshot\"} 0"));
    assert!(!text.contains("grove_object_backlog_items{"));
}

#[derive(Debug)]
struct TestClock(AtomicI64);
impl grove_core::time::Clock for TestClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(self.0.load(Ordering::SeqCst), 0).unwrap()
    }
}

fn database_values(metrics: &Metrics) -> Vec<String> {
    metrics
        .render()
        .unwrap()
        .lines()
        .filter(|line| {
            [
                "grove_database_snapshot",
                "grove_object_backlog",
                "grove_retention_",
                "grove_db_table_",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        })
        .map(str::to_owned)
        .collect()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn real_refresh_times_out_at_ten_seconds_preserves_cache_and_recovers(
    options: sqlx::postgres::PgPoolOptions,
    connect: sqlx::postgres::PgConnectOptions,
) {
    let pool = options
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(60))
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(connect)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE OR REPLACE FUNCTION grove_time.transaction_now()
        RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT '2026-01-01T00:00:00Z'::timestamptz $$;")
        .execute(&pool).await.unwrap();
    let clock = Arc::new(TestClock(AtomicI64::new(1000)));
    let metrics = Metrics::new(clock.clone(), None);
    metrics.register_worker(&super::super::WorkerLabels {
        worker: "database",
        task: "snapshot",
    });
    refresh(&pool, policy(), &metrics).await;
    let baseline = database_values(&metrics);
    assert!(
        baseline
            .iter()
            .any(|line| line == "grove_database_snapshot_timestamp_seconds 1767225600")
    );
    let worker = super::super::WorkerLabels {
        worker: "database",
        task: "snapshot",
    };
    assert_eq!(
        metrics.0.worker_last_success.get_or_create(&worker).get(),
        1000
    );

    // Hold the sole connection so the outer deadline wins over the pool timeout.
    let held = pool.acquire().await.unwrap();
    clock.0.store(2000, Ordering::SeqCst);
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let mut attempt = Box::pin(refresh(&pool, policy(), &metrics));
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(attempt.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    tokio::time::advance(Duration::from_millis(9999)).await;
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(attempt.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    tokio::time::advance(Duration::from_millis(1)).await;
    attempt.await;
    // Tokio timers round to millisecond ticks; the deadline itself remains ten seconds.
    assert!(started.elapsed() >= Duration::from_secs(10));
    assert!(started.elapsed() <= Duration::from_millis(10001));
    tokio::time::resume();
    assert_eq!(database_values(&metrics), baseline);
    assert_eq!(
        metrics.0.worker_last_success.get_or_create(&worker).get(),
        1000
    );
    assert_eq!(metrics.0.worker_active.get_or_create(&worker).get(), 0);
    assert!(metrics.render().unwrap().contains(
        "grove_worker_runs_total{worker=\"database\",task=\"snapshot\",outcome=\"timeout\"} 1"
    ));
    drop(held);

    sqlx::raw_sql("CREATE OR REPLACE FUNCTION grove_time.transaction_now()
        RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT '2026-01-02T00:00:00Z'::timestamptz $$;
        INSERT INTO management.security_events(created_at,actor_kind,request_id,surface,event_type,reason_code)
        VALUES('2025-01-01','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated');
        ALTER TABLE management.security_events RENAME TO private_table_name;")
        .execute(&pool).await.unwrap();
    refresh(&pool, policy(), &metrics).await;
    assert_eq!(database_values(&metrics), baseline);
    assert_eq!(
        metrics.0.worker_last_success.get_or_create(&worker).get(),
        1000
    );
    assert!(metrics.render().unwrap().contains(
        "grove_worker_runs_total{worker=\"database\",task=\"snapshot\",outcome=\"error\"} 1"
    ));
    sqlx::query("ALTER TABLE management.private_table_name RENAME TO security_events")
        .execute(&pool)
        .await
        .unwrap();
    refresh(&pool, policy(), &metrics).await;
    let updated = database_values(&metrics);
    assert!(
        updated
            .iter()
            .any(|line| line == "grove_database_snapshot_timestamp_seconds 1767312000")
    );
    assert!(
        updated
            .iter()
            .any(|line| line == "grove_retention_overdue_rows{stream=\"security_events\"} 1")
    );
    assert_eq!(
        metrics.0.worker_last_success.get_or_create(&worker).get(),
        2000
    );
    assert_eq!(metrics.0.worker_active.get_or_create(&worker).get(), 0);
    assert!(metrics.render().unwrap().contains(
        "grove_worker_runs_total{worker=\"database\",task=\"snapshot\",outcome=\"success\"} 2"
    ));
    assert!(metrics.render().unwrap().contains(
        "grove_worker_runs_total{worker=\"database\",task=\"snapshot\",outcome=\"interrupted\"} 0"
    ));
    assert_ne!(updated, baseline);
    pool.close().await;
}
