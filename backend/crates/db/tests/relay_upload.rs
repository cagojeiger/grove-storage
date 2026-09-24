#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;

use filegate_db::files;
use sqlx::PgPool;
use std::time::Duration;

async fn wait_for_lock(pool: &PgPool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
                 WHERE datname = current_database() AND cardinality(pg_blocking_pids(pid)) > 0)",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("competing transition must wait on upload ownership");
}

async fn relay(pool: &PgPool) -> files::CreatedFile {
    lifecycle::wire(pool, 1000).await;
    let file = lifecycle::create_ok(pool, 4).await;
    files::attach_write_secret(pool, file.lease_id, &format!("sha256:{}", "a".repeat(64)))
        .await
        .unwrap();
    file
}

#[sqlx::test(migrations = "./migrations")]
async fn competing_upload_is_rejected_and_retry_reads_published_measurements(pool: PgPool) {
    let file = relay(&pool).await;
    let claim = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    assert!(claim.recorded.is_none());
    assert!(
        files::claim_relay_upload(&pool, file.file_id, file.lease_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        files::recorded_upload(&pool, file.file_id)
            .await
            .unwrap()
            .is_none()
    );
    claim.done(4, "digest-a", 900).await.unwrap();
    let retry = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry.recorded, Some((4, "digest-a".to_owned())));
    drop(retry);
    assert!(
        files::finalize_commit(&pool, file.file_id, "digest-a")
            .await
            .unwrap()
    );
    assert!(
        files::claim_relay_upload(&pool, file.file_id, file.lease_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_before_admission_cannot_claim(pool: PgPool) {
    let file = relay(&pool).await;
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(file.lease_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        files::claim_relay_upload(&pool, file.file_id, file.lease_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn receive_past_ttl_blocks_reclaim_and_renews_on_finish(pool: PgPool) {
    let file = relay(&pool).await;
    sqlx::query(
        "UPDATE leases SET expires_at = clock_timestamp() + interval '1 second' WHERE id = $1",
    )
    .bind(file.lease_id)
    .execute(&pool)
    .await
    .unwrap();
    let claim = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let candidate = files::expired_pending(&pool, 10)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let worker_pool = pool.clone();
    let reclaim = tokio::spawn(async move {
        files::finalize_reclaim(&worker_pool, &candidate)
            .await
            .unwrap()
    });
    wait_for_lock(&pool).await;
    assert!(!reclaim.is_finished());
    claim.done(4, "digest-a", 900).await.unwrap();
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), reclaim)
            .await
            .unwrap()
            .unwrap()
    );
    assert!(files::expired_pending(&pool, 10).await.unwrap().is_empty());
    assert!(
        files::finalize_commit(&pool, file.file_id, "digest-a")
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn abandoned_claim_leaves_no_measurements_and_allows_retry(pool: PgPool) {
    let file = relay(&pool).await;
    let claim = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    drop(claim);
    assert!(
        files::recorded_upload(&pool, file.file_id)
            .await
            .unwrap()
            .is_none()
    );
    let retry = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    assert!(retry.recorded.is_none());
    retry.done(4, "digest-b", 900).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn commit_waits_for_retry_to_release_ownership(pool: PgPool) {
    let file = relay(&pool).await;
    files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap()
        .done(4, "digest-a", 900)
        .await
        .unwrap();
    let retry = files::claim_relay_upload(&pool, file.file_id, file.lease_id)
        .await
        .unwrap()
        .unwrap();
    let worker_pool = pool.clone();
    let commit = tokio::spawn(async move {
        files::finalize_commit(&worker_pool, file.file_id, "digest-a")
            .await
            .unwrap()
    });
    wait_for_lock(&pool).await;
    assert!(!commit.is_finished());
    drop(retry);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), commit)
            .await
            .unwrap()
            .unwrap()
    );
}
