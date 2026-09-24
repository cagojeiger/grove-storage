#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/lock_wait.rs"]
mod lock_wait;
use filegate_db::{
    files,
    registry::{self, UpdateStorageOutcome as Outcome},
};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn update_rechecks_locations_after_waiting_for_reservation(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let mut reservation = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *reservation)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM storages WHERE id='s' FOR SHARE")
        .execute(&mut *reservation)
        .await
        .unwrap();
    let other = pool.clone();
    let update = tokio::spawn(async move {
        let mut row = lifecycle::s3_row("s", 1000);
        row.bucket = Some("other".into());
        registry::update_storage(&other, &row).await.unwrap()
    });
    lock_wait::wait_for_blocker(&pool, pid).await;
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO files (client_id, declared_size) VALUES ('c', 0) RETURNING id",
    )
    .fetch_one(&mut *reservation)
    .await
    .unwrap();
    sqlx::query("INSERT INTO locations (file_id, storage_id, object_key) VALUES ($1, 's', 'key')")
        .bind(id)
        .execute(&mut *reservation)
        .await
        .unwrap();
    reservation.commit().await.unwrap();
    assert_eq!(update.await.unwrap(), Outcome::LocationInUse);
}

#[sqlx::test(migrations = "./migrations")]
async fn reservation_reads_replaced_address_after_waiting_for_update(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let mut update = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *update)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM storages WHERE id='s' FOR UPDATE")
        .execute(&mut *update)
        .await
        .unwrap();
    let other = pool.clone();
    let create = tokio::spawn(async move { lifecycle::create_ok(&other, 10).await });
    lock_wait::wait_for_blocker(&pool, pid).await;
    sqlx::query("UPDATE storages SET bucket='other' WHERE id='s'")
        .execute(&mut *update)
        .await
        .unwrap();
    update.commit().await.unwrap();
    let created = create.await.unwrap();
    assert_eq!(created.storage.bucket.as_deref(), Some("other"));
    assert!(
        files::access(&pool, "c", created.file_id)
            .await
            .unwrap()
            .is_some()
    );
}
