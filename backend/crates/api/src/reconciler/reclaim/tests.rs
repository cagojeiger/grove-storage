#![allow(clippy::unwrap_used, clippy::panic)]

use filegate_db::{files, registry};
use sqlx::PgPool;

#[sqlx::test(migrations = "../db/migrations")]
async fn filesystem_delete_failure_is_retried_by_the_real_worker(pool: PgPool) {
    let state = crate::routes::tests::test_state();
    let root = std::env::temp_dir().join(format!("grove-reclaim-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&root).await.unwrap();
    sqlx::query(
        "INSERT INTO storages (id, kind, root_path, capacity_bytes) VALUES ('s', 'fs', $1, 1000)",
    )
    .bind(root.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    registry::insert_client(&pool, "c", "s").await.unwrap();
    let file = match files::create(
        &pool,
        files::CreateSpec {
            client_id: "c",
            declared_size: 10,
            content_type: None,
            declared_md5: None,
            lease_ttl_secs: 900,
            part_size: None,
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

    // remove_file cannot delete a directory: exercise the real fs adapter error.
    let object = root.join(&file.object_key);
    tokio::fs::create_dir_all(&object).await.unwrap();
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert!(object.is_dir());
    assert_eq!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(files::prune_terminal_leases(&pool, 0, 20).await.unwrap(), 0);

    // Repair the backend, then run the same worker again without changing the DB.
    tokio::fs::remove_dir(&object).await.unwrap();
    tokio::fs::write(&object, b"remaining").await.unwrap();
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert!(!object.exists());
    assert!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .is_empty()
    );
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    tokio::fs::remove_dir_all(&root).await.unwrap();
}
