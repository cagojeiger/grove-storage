//! Characterization of the existing generic reclaim policy, not the desired
//! recovery contract. Replace these expectations when durable retry is added.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/lifecycle.rs"]
mod lifecycle;

use filegate_db::{files, registry, registry::UpdateStorageOutcome};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn failed_physical_cleanup_loses_location_and_future_retry_candidate(pool: PgPool) {
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
    assert_eq!(locations, 0);
    assert!(files::expired_pending(&pool, 20).await.unwrap().is_empty());
    assert!(files::purgeable(&pool, 20).await.unwrap().is_empty());
    assert_eq!(lifecycle::observed(&pool).await, (0, 0, 0));

    // The address guard also has no remaining location to protect.
    let mut replacement = lifecycle::s3_row("s", 1000);
    replacement.bucket = Some("replacement".to_owned());
    assert_eq!(
        registry::update_storage(&pool, &replacement).await.unwrap(),
        UpdateStorageOutcome::Updated
    );
}
