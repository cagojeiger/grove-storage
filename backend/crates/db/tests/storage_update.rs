#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;
use filegate_db::{
    files,
    registry::{self, UpdateStorageOutcome as Outcome},
};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn missing_and_empty_storage_have_distinct_outcomes(pool: PgPool) {
    let mut row = lifecycle::s3_row("s", 1000);
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::NotFound
    );
    lifecycle::wire(&pool, 1000).await;
    row.bucket = Some("other".into());
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::Updated
    );
    assert_eq!(
        registry::get_storage(&pool, "s")
            .await
            .unwrap()
            .unwrap()
            .bucket,
        row.bucket
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn all_address_fields_are_protected_even_for_zero_byte_pending_files(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    lifecycle::create_ok(&pool, 0).await;
    let original = lifecycle::s3_row("s", 1000);
    let mut variants = Vec::new();
    let mut row = original.clone();
    row.bucket = Some("other".into());
    variants.push(row);
    let mut row = original.clone();
    row.endpoint = Some("http://other".into());
    variants.push(row);
    let mut row = original.clone();
    row.public_endpoint = Some("http://public".into());
    variants.push(row);
    let mut row = original.clone();
    row.region = Some("other-region".into());
    variants.push(row);
    let mut row = original.clone();
    row.force_path_style = false;
    variants.push(row);
    let mut row = original.clone();
    row.kind = "fs".into();
    variants.push(row);
    for row in variants {
        assert_eq!(
            registry::update_storage(&pool, &row).await.unwrap(),
            Outcome::LocationInUse
        );
    }
    let actual = registry::get_storage(&pool, "s").await.unwrap().unwrap();
    assert_eq!(actual.bucket, original.bucket);
    assert_eq!(actual.endpoint, original.endpoint);
}

#[sqlx::test(migrations = "./migrations")]
async fn deleted_files_remain_protected_until_purge(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    sqlx::query(
        "UPDATE files SET state = 'active', etag = 'etag', committed_at = now() WHERE id = $1",
    )
    .bind(file.file_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut row = lifecycle::s3_row("s", 1000);
    row.bucket = Some("other".into());
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::LocationInUse
    );
    files::mark_deleted(&pool, "c", file.file_id).await.unwrap();
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::LocationInUse
    );
    let candidates = files::purgeable(&pool, 10).await.unwrap();
    files::finalize_purge(&pool, candidates.first().unwrap())
        .await
        .unwrap();
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::Updated
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn credentials_capacity_and_relay_can_change_with_locations(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    lifecycle::create_ok(&pool, 10).await;
    let mut row = lifecycle::s3_row("s", 2000);
    row.access_key = Some("new-key".into());
    row.secret_key_ciphertext = Some(vec![4, 5, 6]);
    row.secret_key_nonce = Some(vec![1; 12]);
    row.enc_key_id = Some("v2".into());
    row.force_relay = true;
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::Updated
    );
    let actual = registry::get_storage(&pool, "s").await.unwrap().unwrap();
    assert_eq!(actual.access_key, row.access_key);
    assert_eq!(actual.secret_key_ciphertext, row.secret_key_ciphertext);
    assert_eq!(actual.capacity_bytes, 2000);
    assert!(actual.force_relay);
}

#[sqlx::test(migrations = "./migrations")]
async fn filesystem_root_is_protected(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    sqlx::query("UPDATE storages SET kind='fs', root_path='/data', endpoint=NULL, public_endpoint=NULL, region=NULL, bucket=NULL, force_path_style=false, access_key=NULL, secret_key_ciphertext=NULL, secret_key_nonce=NULL, enc_key_id=NULL WHERE id='s'")
        .execute(&pool).await.unwrap();
    lifecycle::create_ok(&pool, 10).await;
    let mut row = registry::get_storage(&pool, "s").await.unwrap().unwrap();
    row.root_path = Some("/different".into());
    assert_eq!(
        registry::update_storage(&pool, &row).await.unwrap(),
        Outcome::LocationInUse
    );
}
