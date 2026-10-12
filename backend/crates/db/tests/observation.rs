#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/s3_single.rs"]
mod s3_single;

use grove_db::{files, observation};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn normal_pending_files_and_terminal_history_are_not_cleanup_queue(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 100).await;
    assert!(
        observation::recovery_queues(&pool)
            .await
            .unwrap()
            .iter()
            .all(|q| q.objects == 0)
    );
    sqlx::query("UPDATE files SET state='deleted',committed_at=now(),deleted_at=now() WHERE id=$1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let queue = observation::recovery_queues(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|q| q.job == "cleanup")
        .unwrap();
    assert_eq!(queue.objects, 1);
    assert!(queue.oldest_at.is_some());
    sqlx::query("DELETE FROM locations WHERE file_id=$1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        observation::recovery_queues(&pool)
            .await
            .unwrap()
            .iter()
            .all(|q| q.objects == 0)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn retry_backoff_does_not_hide_cleanup_or_reset_its_age(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 100).await;
    sqlx::query("UPDATE files SET state='deleted',committed_at=now()-interval '2 hours',deleted_at=now()-interval '1 hour',recovery_after=now()+interval '1 minute' WHERE id=$1")
        .bind(file.file_id).execute(&pool).await.unwrap();
    let queue = observation::recovery_queues(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|q| q.job == "cleanup")
        .unwrap();
    assert_eq!(queue.objects, 1);
    let age: i64 = sqlx::query_scalar("SELECT extract(epoch FROM (now()-$1))::bigint")
        .bind(queue.oldest_at.unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(age >= 3599);
}

#[sqlx::test(migrations = "./migrations")]
async fn missing_cleanup_age_is_unknown_not_new_work(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 100).await;
    sqlx::query("UPDATE files SET state='reclaimed' WHERE id=$1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let queue = observation::recovery_queues(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|q| q.job == "cleanup")
        .unwrap();
    assert_eq!(queue.objects, 1);
    assert_eq!(queue.unknown_age_objects, 1);
    assert!(queue.oldest_at.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn snapshot_is_bounded_under_table_contention_and_does_not_leak_session_settings(
    pool: PgPool,
) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 100).await;
    let (usage, _) = observation::snapshot(&pool).await.unwrap();
    assert_eq!(usage.iter().map(|u| u.reserved_bytes).sum::<i64>(), 100);
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE files IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let error = observation::snapshot(&pool).await.err().unwrap();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55P03")
    );
    lock.rollback().await.unwrap();
    assert_eq!(
        files::stat(&pool, "c", file.file_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        "pending"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SHOW transaction_read_only")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "off"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SHOW statement_timeout")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "0"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn unexpired_upload_owner_is_not_recovery_backlog(pool: PgPool) {
    s3_single::wire(&pool).await;
    let file = s3_single::create_s3_upload(&pool, "object").await;
    s3_single::claim_single(&pool, "object", file.file_id, "etag").await;
    assert!(
        observation::recovery_queues(&pool)
            .await
            .unwrap()
            .iter()
            .all(|q| q.objects == 0)
    );
    sqlx::query(
        "UPDATE leases SET expires_at=now()-interval '1 minute' WHERE file_id=$1 AND kind='write'",
    )
    .bind(file.file_id)
    .execute(&pool)
    .await
    .unwrap();
    let queue = observation::recovery_queues(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|q| q.job == "completion")
        .unwrap();
    assert_eq!(queue.objects, 1);
    assert!(queue.oldest_at.is_some());
    assert_eq!(
        files::stat(&pool, "c", file.file_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        "pending"
    );
}
