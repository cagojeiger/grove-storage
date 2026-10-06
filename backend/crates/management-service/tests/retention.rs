#![allow(clippy::unwrap_used)]
#[path = "../../db/tests/support/lifecycle.rs"]
#[allow(clippy::panic)]
mod lifecycle;
mod support;

use filegate_core::ManagementLogRetention;
use filegate_db::{PgPool, management as db};
use grove_management_service::retention;
use std::{num::NonZeroU16, time::Duration};

fn policy() -> ManagementLogRetention {
    ManagementLogRetention {
        audit_days: NonZeroU16::new(365).unwrap(),
        security_days: NonZeroU16::new(90).unwrap(),
        invocation_days: NonZeroU16::new(30).unwrap(),
    }
}

async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO management.audit_events(created_at,actor_kind,request_id,surface,action,resource_type,resource_id)
        SELECT now()-d*interval '1 day','system',gen_random_uuid(),'console','storage.create','storage','historic' FROM unnest(ARRAY[31,91,366]) d;
        INSERT INTO management.security_events(created_at,actor_kind,request_id,surface,event_type,reason_code)
        SELECT now()-d*interval '1 day','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated' FROM unnest(ARRAY[31,91,366]) d;
        INSERT INTO management.command_invocations(created_at,actor_kind,request_id,surface,operation,outcome,duration_ms)
        SELECT now()-d*interval '1 day','master',gen_random_uuid(),'console','storage.list','succeeded',1 FROM unnest(ARRAY[31,91,366]) d;")
        .execute(pool).await.unwrap();
}

async fn counts(pool: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM management.audit_events),
        (SELECT count(*) FROM management.security_events),
        (SELECT count(*) FROM management.command_invocations)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn periods_are_independent_and_identity_files_and_keys_are_untouched(pool: PgPool) {
    let admin = support::owner(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    let client_key = filegate_core::client_key_hash("retention-test-client");
    filegate_db::registry::insert_client_key(&pool, "c", &client_key)
        .await
        .unwrap();
    let audit_before = support::audit_count(&pool).await;
    seed(&pool).await;
    retention::run(&pool, policy()).await;
    assert_eq!(counts(&pool).await, (audit_before + 2, 1, 0));
    assert!(
        db::session_actor(&pool, &admin.session)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(lifecycle::observed(&pool).await, (10, 0, 0));
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM files WHERE id=$1)")
            .bind(file.file_id)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM client_keys WHERE key_hash=$1)")
            .bind(client_key)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    // Maintenance must not record its own invocations or audit events.
    retention::run(&pool, policy()).await;
    assert_eq!(counts(&pool).await, (audit_before + 2, 1, 0));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn each_run_is_bounded_and_backlog_drains_on_later_ticks(pool: PgPool) {
    sqlx::raw_sql("INSERT INTO management.command_invocations(created_at,actor_kind,request_id,surface,operation,outcome,duration_ms)
        SELECT now()-interval '31 days','master',gen_random_uuid(),'cli','storage.list','succeeded',1 FROM generate_series(1,2001);
        INSERT INTO management.command_invocations(actor_kind,request_id,surface,operation,outcome,duration_ms)
        VALUES('master',gen_random_uuid(),'cli','storage.list','succeeded',1);")
        .execute(&pool).await.unwrap();
    for remaining in [1002, 2, 1, 1] {
        retention::run(&pool, policy()).await;
        assert_eq!(counts(&pool).await, (0, 0, remaining));
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn locked_stream_is_bounded_other_streams_continue_and_retry_succeeds(pool: PgPool) {
    seed(&pool).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE management.audit_events IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *fence)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), retention::run(&pool, policy()))
        .await
        .unwrap();
    fence.rollback().await.unwrap();
    assert_eq!(counts(&pool).await, (3, 1, 0));
    retention::run(&pool, policy()).await;
    assert_eq!(counts(&pool).await, (2, 1, 0));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn retention_lock_selects_one_runner_and_releases_for_next_tick(pool: PgPool) {
    seed(&pool).await;
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    let runner = {
        let pool = pool.clone();
        let entered = entered.clone();
        let release = release.clone();
        tokio::spawn(async move {
            db::retention::with_lock(&pool, || async {
                entered.notify_one();
                release.notified().await;
                retention::run(&pool, policy()).await;
            })
            .await
            .unwrap()
        })
    };
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    let executed = std::sync::atomic::AtomicBool::new(false);
    assert!(
        db::retention::with_lock(&pool, || async {
            executed.store(true, std::sync::atomic::Ordering::SeqCst);
            retention::run(&pool, policy()).await;
        })
        .await
        .unwrap()
        .is_none()
    );
    assert!(!executed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(counts(&pool).await, (3, 3, 3));
    release.notify_one();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), runner)
            .await
            .unwrap()
            .unwrap()
            .is_some()
    );
    assert_eq!(counts(&pool).await, (2, 1, 0));
    assert!(
        db::retention::with_lock(&pool, || retention::run(&pool, policy()))
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn cancelled_runner_releases_lock_and_retention_works_with_two_connections(pool: PgPool) {
    seed(&pool).await;
    let small_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let runner = {
        let pool = small_pool.clone();
        let entered = entered.clone();
        tokio::spawn(async move {
            db::retention::with_lock(&pool, || async {
                entered.notify_one();
                std::future::pending::<()>().await;
            })
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    runner.abort();
    assert!(runner.await.unwrap_err().is_cancelled());
    // SQLx queues rollback on transaction drop; the next tick must regain the lock.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if db::retention::with_lock(&small_pool, || retention::run(&small_pool, policy()))
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(counts(&pool).await, (2, 1, 0));
    small_pool.close().await;
}

#[sqlx::test(migrations = "../db/migrations")]
async fn object_lock_does_not_block_retention_and_retention_does_not_block_objects(pool: PgPool) {
    seed(&pool).await;
    let object_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let maintenance_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    let object_worker = {
        let pool = object_pool.clone();
        let entered = entered.clone();
        let release = release.clone();
        tokio::spawn(async move {
            filegate_db::with_reconciler_lock(&pool, || async {
                entered.notify_one();
                release.notified().await;
            })
            .await
            .unwrap()
        })
    };
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    // Exhaust the object pool: retention must not need any of these connections.
    let occupied = object_pool.acquire().await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(3),
            db::retention::with_lock(&maintenance_pool, || retention::run(
                &maintenance_pool,
                policy()
            ))
        )
        .await
        .unwrap()
        .unwrap()
        .is_some()
    );
    assert_eq!(counts(&pool).await, (2, 1, 0));
    drop(occupied);
    release.notify_one();
    assert!(object_worker.await.unwrap().is_some());
    assert!(
        db::retention::with_lock(&maintenance_pool, || async {
            assert!(
                filegate_db::with_reconciler_lock(&object_pool, || async {})
                    .await
                    .unwrap()
                    .is_some()
            );
        })
        .await
        .unwrap()
        .is_some()
    );
    object_pool.close().await;
    maintenance_pool.close().await;
}
