//! S3 multipart DB integration tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "support/s3_multipart.rs"]
mod support;

use grove_db::files;
use grove_db::s3_registry as s3;
use sqlx::PgPool;
use support::{KEY, file_row, open_multipart, upload_part, wire};

// ── abort → reclaim ────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn abort_keeps_recovery_material_until_cleanup_is_confirmed(pool: PgPool) {
    wire(&pool).await;
    let created = open_multipart(&pool).await;
    upload_part(&pool, &created, 1, 50, "aaaa").await;
    // Abort는 먼저 aborting만 선점한다. 외부 정리가 실패한 것으로 모사해
    // finalize하지 않으면 session/location/lease가 다음 재시도 재료로 남는다.
    assert_eq!(
        s3::claim_abort(&pool, "c", KEY, created.file_id)
            .await
            .unwrap(),
        s3::AbortClaim::Claimed
    );
    let (state, _, _) = file_row(&pool, created.file_id).await;
    assert_eq!(state, "pending");
    let location: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT file_id FROM locations WHERE file_id = $1")
            .bind(created.file_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(location, Some(created.file_id));
    let cleanup = s3::cleanup_candidates(&pool, 10).await.unwrap();
    assert_eq!(cleanup.len(), 1);
    assert_eq!(cleanup[0].file_id, created.file_id);
    assert_eq!(cleanup[0].write_lease_id, Some(created.lease_id));
    assert!(
        !files::reclaim_pending(&pool, created.file_id)
            .await
            .unwrap()
    );

    // 물리 정리 성공 뒤에만 DB 회수가 session/location을 제거한다.
    assert!(s3::finalize_abort(&pool, created.file_id).await.unwrap());
    let (state, _, _) = file_row(&pool, created.file_id).await;
    assert_eq!(state, "reclaimed");
    let location: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT file_id FROM locations WHERE file_id = $1")
            .bind(created.file_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(location.is_none());
    let lease_state: String = sqlx::query_scalar("SELECT state FROM leases WHERE id = $1")
        .bind(created.lease_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lease_state, "expired");
    assert!(!s3::finalize_abort(&pool, created.file_id).await.unwrap());
    let session_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM s3_uploads WHERE file_id = $1")
            .bind(created.file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(session_count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn abort_after_complete_does_not_reclaim(pool: PgPool) {
    // 이미 Complete된(active) 세션의 Abort는 회수하지 않는다 — pending만 회수.
    wire(&pool).await;
    let created = open_multipart(&pool).await;
    assert_eq!(
        s3::claim_completion(
            &pool,
            s3::CompletionSpec {
                client_id: "c",
                key: KEY,
                file_id: created.file_id,
                multipart: true,
                expected_size: 10,
                expected_etag: "e-1",
                lease_ttl_secs: 900,
            },
        )
        .await
        .unwrap(),
        s3::CompletionClaim::Claimed
    );
    s3::finalize_multipart_upload(&pool, "c", KEY, created.file_id)
        .await
        .unwrap();
    assert_eq!(
        s3::claim_abort(&pool, "c", KEY, created.file_id)
            .await
            .unwrap(),
        s3::AbortClaim::Unavailable
    );
    let (state, _, _) = file_row(&pool, created.file_id).await;
    assert_eq!(state, "active");
}

#[sqlx::test(migrations = "./migrations")]
async fn wrong_key_complete_cannot_activate_or_map_upload(pool: PgPool) {
    wire(&pool).await;
    let created = open_multipart(&pool).await;

    assert_eq!(
        s3::claim_completion(
            &pool,
            s3::CompletionSpec {
                client_id: "c",
                key: "dir/other.bin",
                file_id: created.file_id,
                multipart: true,
                expected_size: 10,
                expected_etag: "e-1",
                lease_ttl_secs: 900,
            },
        )
        .await
        .unwrap(),
        s3::CompletionClaim::Unavailable
    );
    assert_eq!(
        s3::finalize_multipart_upload(&pool, "c", "dir/other.bin", created.file_id)
            .await
            .unwrap(),
        s3::FinalizeOutcome::NotPending
    );
    let (state, size, etag) = file_row(&pool, created.file_id).await;
    assert_eq!(state, "pending");
    assert_eq!(size, 0);
    assert!(etag.is_none());
    assert!(s3::get_key(&pool, "c", KEY).await.unwrap().is_none());
    assert!(
        s3::get_key(&pool, "c", "dir/other.bin")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        s3::discard_unstarted_upload(&pool, created.file_id)
            .await
            .unwrap()
    );
}

// ── reconciler 회수 재료 ───────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn expired_multipart_preserves_recovery_material_until_abort_finishes(pool: PgPool) {
    wire(&pool).await;
    let created = open_multipart(&pool).await;
    files::attach_upload_id(&pool, created.lease_id, "vendor-upload")
        .await
        .unwrap();
    // 만료되면 reconciler의 만료 회수가 줍는다 (벤더 Abort 재료 포함).
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 hour' WHERE file_id = $1")
        .bind(created.file_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(files::expired_pending(&pool, 10).await.unwrap().is_empty());
    assert_eq!(
        s3::expired_open_uploads(&pool, 10).await.unwrap(),
        vec![created.file_id]
    );
    assert!(
        s3::claim_expired_abort(&pool, created.file_id)
            .await
            .unwrap()
    );
    let cleanup = s3::cleanup_candidates(&pool, 10).await.unwrap();
    assert_eq!(cleanup.len(), 1);
    assert_eq!(cleanup[0].file_id, created.file_id);
    assert!(cleanup[0].multipart);
    assert_eq!(cleanup[0].upload_id.as_deref(), Some("vendor-upload"));
    assert_eq!(cleanup[0].write_lease_id, Some(created.lease_id));
    // aborting의 expired lease는 보존 기간이 지나도 session이 소유한 복구
    // 핸들이므로 GC되지 않는다.
    sqlx::query("UPDATE leases SET created_at = now() - interval '2 days' WHERE id = $1")
        .bind(created.lease_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        files::prune_terminal_leases(&pool, 24 * 3600, 10)
            .await
            .unwrap(),
        0
    );
    assert!(
        files::write_lease(&pool, created.file_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(s3::finalize_abort(&pool, created.file_id).await.unwrap());
}
