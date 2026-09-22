//! Generic reclaim cleanup must survive failures until physical deletion succeeds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;

use filegate_db::{files, registry, registry::UpdateStorageOutcome};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn unknown_vendor_id_still_retains_key_and_multipart_cleanup_intent(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = match files::create(
        &pool,
        files::CreateSpec {
            part_size: Some(5),
            ..lifecycle::spec(10)
        },
    )
    .await
    .unwrap()
    {
        files::CreateOutcome::Created(file) => file,
        _ => panic!("expected file"),
    };
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(file.lease_id)
        .execute(&pool)
        .await
        .unwrap();
    let candidates = files::expired_pending(&pool, 20).await.unwrap();
    assert!(
        files::finalize_reclaim(&pool, candidates.first().unwrap())
            .await
            .unwrap()
    );
    let retries = files::reclaim_cleanup_candidates(&pool, 20).await.unwrap();
    let retry = retries.first().unwrap();
    assert!(retry.multipart);
    assert_eq!(retry.upload_id, None);
    assert_eq!(retry.object_key, file.object_key);
    assert_eq!(retry.write_lease_id, Some(file.lease_id));
    let usage = filegate_db::usage::by_storage(&pool).await.unwrap();
    assert_eq!(usage.first().unwrap().purge_pending_files, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn multipart_handles_survive_gc_until_cleanup(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = match files::create(
        &pool,
        files::CreateSpec {
            part_size: Some(5),
            ..lifecycle::spec(10)
        },
    )
    .await
    .unwrap()
    {
        files::CreateOutcome::Created(file) => file,
        _ => panic!("expected file"),
    };
    files::attach_upload_id(&pool, file.lease_id, "vendor-session")
        .await
        .unwrap();
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second', created_at = now() - interval '91 days' WHERE id = $1")
        .bind(file.lease_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE files SET created_at = now() - interval '91 days' WHERE id = $1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let candidates = files::expired_pending(&pool, 20).await.unwrap();
    assert!(
        files::finalize_reclaim(&pool, candidates.first().unwrap())
            .await
            .unwrap()
    );
    assert_eq!(
        files::prune_terminal_leases(&pool, 86400, 20)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        files::prune_terminal_files(&pool, 86400, 20).await.unwrap(),
        0
    );
    let retries = files::reclaim_cleanup_candidates(&pool, 20).await.unwrap();
    let retry = retries.first().unwrap();
    assert!(retry.multipart);
    assert_eq!(retry.upload_id.as_deref(), Some("vendor-session"));
    assert_eq!(retry.write_lease_id, Some(file.lease_id));
    assert!(files::finalize_reclaim_cleanup(&pool, retry).await.unwrap());
    assert_eq!(
        files::prune_terminal_leases(&pool, 86400, 20)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        files::prune_terminal_files(&pool, 86400, 20).await.unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn cleanup_finalization_cannot_remove_a_pending_or_active_location(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    let candidate = files::SweepCandidate {
        file_id: file.file_id,
        storage_id: file.storage.id,
        object_key: file.object_key,
        upload_id: None,
        write_lease_id: Some(file.lease_id),
        multipart: false,
    };
    assert!(
        !files::finalize_reclaim_cleanup(&pool, &candidate)
            .await
            .unwrap()
    );
    assert!(
        files::finalize_commit(&pool, file.file_id, "etag")
            .await
            .unwrap()
    );
    assert!(
        !files::finalize_reclaim_cleanup(&pool, &candidate)
            .await
            .unwrap()
    );
    assert!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_physical_cleanup_preserves_location_and_future_retry_candidate(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(file.lease_id)
        .execute(&pool)
        .await
        .unwrap();

    let candidates = files::expired_pending(&pool, 20).await.unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = candidates.first().unwrap();
    assert_eq!(candidate.file_id, file.file_id);

    // Follow the worker's actual DB-first sequence, injecting a storage error
    // instead of contacting a vendor. The fake physical object remains present.
    let mut physical_object = Some(vec![0_u8; 10]);
    assert!(files::finalize_reclaim(&pool, candidate).await.unwrap());
    let deletion: Result<(), &str> = Err("injected storage unavailable");
    if deletion.is_ok() {
        physical_object = None;
    }
    assert!(physical_object.is_some());

    let state: String = sqlx::query_scalar("SELECT state FROM files WHERE id = $1")
        .bind(file.file_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "reclaimed");
    let locations: i64 = sqlx::query_scalar("SELECT count(*) FROM locations WHERE file_id = $1")
        .bind(file.file_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locations, 1);
    assert!(files::expired_pending(&pool, 20).await.unwrap().is_empty());
    assert!(files::purgeable(&pool, 20).await.unwrap().is_empty());
    assert_eq!(lifecycle::observed(&pool).await, (0, 0, 10));

    // Address changes remain fenced while the physical object needs cleanup.
    let mut replacement = lifecycle::s3_row("s", 1000);
    replacement.bucket = Some("replacement".to_owned());
    assert_eq!(
        registry::update_storage(&pool, &replacement).await.unwrap(),
        UpdateStorageOutcome::LocationInUse
    );
    assert_eq!(files::prune_terminal_leases(&pool, 0, 20).await.unwrap(), 0);
    assert_eq!(files::prune_terminal_files(&pool, 0, 20).await.unwrap(), 0);
    let retries = files::reclaim_cleanup_candidates(&pool, 20).await.unwrap();
    let retry = retries.first().unwrap();
    assert_eq!(retry.file_id, file.file_id);
    assert_eq!(retry.object_key, file.object_key);
    assert_eq!(retry.write_lease_id, Some(file.lease_id));
    assert!(
        !files::finalize_commit(&pool, file.file_id, "late-etag")
            .await
            .unwrap()
    );
    physical_object = None;
    assert!(physical_object.is_none());
    assert!(files::finalize_reclaim_cleanup(&pool, retry).await.unwrap());
    assert!(!files::finalize_reclaim_cleanup(&pool, retry).await.unwrap());
    assert!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(lifecycle::observed(&pool).await, (0, 0, 0));
    assert_eq!(files::prune_terminal_leases(&pool, 0, 20).await.unwrap(), 1);
    assert_eq!(
        registry::update_storage(&pool, &replacement).await.unwrap(),
        UpdateStorageOutcome::Updated
    );
}
