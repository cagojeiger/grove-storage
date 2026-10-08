#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

#[path = "support/s3_single.rs"]
mod support;

use grove_db::s3_registry as s3;
use sqlx::PgPool;
use support::{claim_single, create_s3_upload, create_s3_upload_with_condition, file_state, wire};

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_creates_publish_one_and_retain_loser_for_cleanup(pool: PgPool) {
    wire(&pool).await;
    let a = create_s3_upload_with_condition(&pool, "k", true).await;
    let b = create_s3_upload_with_condition(&pool, "k", true).await;
    claim_single(&pool, "k", a.file_id, "a").await;
    claim_single(&pool, "k", b.file_id, "b").await;
    let (a_result, b_result) = tokio::join!(
        s3::finalize_single_upload(&pool, "c", "k", a.file_id),
        s3::finalize_single_upload(&pool, "c", "k", b.file_id),
    );
    let results = [a_result.unwrap(), b_result.unwrap()];
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == s3::FinalizeOutcome::Finalized { displaced: None })
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == s3::FinalizeOutcome::PreconditionFailed)
            .count(),
        1
    );
    let winner = s3::get_key(&pool, "c", "k").await.unwrap().unwrap();
    let loser = if winner == a.file_id {
        b.file_id
    } else {
        a.file_id
    };
    assert_eq!(file_state(&pool, winner).await, "active");
    assert_eq!(file_state(&pool, loser).await, "pending");
    let cleanup = s3::cleanup_candidates(&pool, 10).await.unwrap();
    assert_eq!(cleanup.len(), 1);
    assert_eq!(cleanup[0].file_id, loser);
    assert!(s3::finalize_abort(&pool, loser).await.unwrap());
    assert_eq!(file_state(&pool, loser).await, "reclaimed");
    assert_eq!(s3::get_key(&pool, "c", "k").await.unwrap(), Some(winner));
}

#[sqlx::test(migrations = "./migrations")]
async fn recovery_rechecks_durable_condition_and_never_replaces_new_winner(pool: PgPool) {
    wire(&pool).await;
    let abandoned = create_s3_upload_with_condition(&pool, "k", true).await;
    claim_single(&pool, "k", abandoned.file_id, "abandoned").await;
    let winner = create_s3_upload(&pool, "k").await;
    claim_single(&pool, "k", winner.file_id, "winner").await;
    s3::finalize_single_upload(&pool, "c", "k", winner.file_id)
        .await
        .unwrap();
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 hour' WHERE file_id = $1")
        .bind(abandoned.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let candidates = s3::completion_candidates(&pool, 10).await.unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(
        s3::finalize_single_upload(
            &pool,
            &candidate.client_id,
            &candidate.key,
            candidate.file_id
        )
        .await
        .unwrap(),
        s3::FinalizeOutcome::PreconditionFailed
    );
    assert_eq!(
        s3::get_key(&pool, "c", "k").await.unwrap(),
        Some(winner.file_id)
    );
    assert_eq!(file_state(&pool, winner.file_id).await, "active");
    // Once rejected, deleting the winner cannot make recovery publish the loser.
    s3::delete_key(&pool, "c", "k").await.unwrap();
    assert_eq!(
        s3::finalize_single_upload(&pool, "c", "k", abandoned.file_id)
            .await
            .unwrap(),
        s3::FinalizeOutcome::NotPending
    );
    assert!(s3::get_key(&pool, "c", "k").await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_activation_rolls_back_conditional_key_claim(pool: PgPool) {
    wire(&pool).await;
    let upload = create_s3_upload_with_condition(&pool, "k", true).await;
    claim_single(&pool, "k", upload.file_id, "etag").await;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_activation() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'injected activation failure'; END $$;
        CREATE TRIGGER reject_activation BEFORE UPDATE ON files
        FOR EACH ROW WHEN (NEW.state = 'active') EXECUTE FUNCTION reject_activation();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        s3::finalize_single_upload(&pool, "c", "k", upload.file_id)
            .await
            .is_err()
    );
    assert!(s3::get_key(&pool, "c", "k").await.unwrap().is_none());
    assert_eq!(file_state(&pool, upload.file_id).await, "pending");
    let session: (String, bool) =
        sqlx::query_as("SELECT state, if_none_match FROM uploads WHERE file_id = $1")
            .bind(upload.file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(session, ("completing".to_owned(), true));
    sqlx::query("DROP TRIGGER reject_activation ON files")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        s3::finalize_single_upload(&pool, "c", "k", upload.file_id)
            .await
            .unwrap(),
        s3::FinalizeOutcome::Finalized { displaced: None }
    );
}
