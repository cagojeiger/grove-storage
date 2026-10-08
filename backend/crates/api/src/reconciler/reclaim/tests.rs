#![allow(clippy::unwrap_used, clippy::panic)]

use grove_db::{files, registry};
use sqlx::PgPool;

use super::super::tests::time;

#[sqlx::test(migrations = "../db/migrations")]
async fn s3_delete_failure_is_retried_by_the_real_worker(pool: PgPool) {
    time::install_clock(&pool).await;
    use axum::{Router, http::StatusCode};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let state = crate::routes::tests::test_state();
    let failing = Arc::new(AtomicBool::new(true));
    let deleted = Arc::new(AtomicUsize::new(0));
    let fail = failing.clone();
    let count = deleted.clone();
    let provider = Router::new().fallback(move |request: axum::extract::Request| {
        let fail = fail.clone();
        let count = count.clone();
        async move {
            assert_eq!(request.method(), "DELETE");
            if fail.load(Ordering::SeqCst) {
                return StatusCode::FORBIDDEN;
            }
            count.fetch_add(1, Ordering::SeqCst);
            StatusCode::NO_CONTENT
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    let secret = state
        .crypto
        .encrypt("s", &"provider-secret".to_owned().into())
        .unwrap();
    registry::insert_storage(
        &pool,
        &registry::StorageRow {
            id: "s".into(),
            kind: "s3".into(),
            force_relay: false,
            endpoint: Some(endpoint.clone()),
            public_endpoint: Some(endpoint),
            region: Some("local".into()),
            bucket: Some("objects".into()),
            force_path_style: true,
            access_key: Some("key".into()),
            secret_key_ciphertext: Some(secret.ciphertext),
            secret_key_nonce: Some(secret.nonce),
            enc_key_id: Some(state.crypto.active_key_id().into()),
            capacity_bytes: 1000,
        },
    )
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
    sqlx::query("UPDATE leases SET expires_at = grove_time.transaction_now() - interval '1 second' WHERE id = $1")
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

    // A provider denial must retain recovery metadata and the write lease.
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert_eq!(deleted.load(Ordering::SeqCst), 0);
    assert_eq!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .len(),
        0
    );
    assert_eq!(files::prune_terminal_leases(&pool, 0, 20).await.unwrap(), 0);

    // Repair the backend; retry only becomes eligible at the injected deadline.
    failing.store(false, Ordering::SeqCst);
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert_eq!(deleted.load(Ordering::SeqCst), 0);
    time::set_time(&pool, time::base() + chrono::Duration::seconds(30)).await;
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert_eq!(deleted.load(Ordering::SeqCst), 1);
    assert!(
        files::reclaim_cleanup_candidates(&pool, 20)
            .await
            .unwrap()
            .is_empty()
    );
    super::recover(&pool, &state.crypto, &state.s3_clients).await;
    assert_eq!(deleted.load(Ordering::SeqCst), 1);
    task.abort();
}
