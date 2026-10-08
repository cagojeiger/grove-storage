#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;
use axum::{
    Router,
    body::Body,
    http::{Response, StatusCode},
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use uuid::Uuid;

#[path = "../../../db/tests/support/time.rs"]
pub(super) mod time;

async fn register(pool: &PgPool, crypto: &Crypto, endpoint: &str) {
    let secret = crypto
        .encrypt("s", &"provider-secret".to_owned().into())
        .unwrap();
    registry::insert_storage(
        pool,
        &registry::StorageRow {
            id: "s".into(),
            kind: "s3".into(),
            force_relay: false,
            endpoint: Some(endpoint.into()),
            public_endpoint: Some(endpoint.into()),
            region: Some("local".into()),
            bucket: Some("objects".into()),
            force_path_style: true,
            access_key: Some("key".into()),
            secret_key_ciphertext: Some(secret.ciphertext),
            secret_key_nonce: Some(secret.nonce),
            enc_key_id: Some(crypto.active_key_id().into()),
            capacity_bytes: 10_000,
        },
    )
    .await
    .unwrap();
    registry::insert_client(pool, "c", "s").await.unwrap();
}

async fn seed(pool: &PgPool, state: &str, ids: &[Uuid]) {
    sqlx::query(
        "INSERT INTO files(id,client_id,state,declared_size,committed_at,deleted_at)
        SELECT id,'c',$2,10,CASE WHEN $2='deleted' THEN grove_time.transaction_now() END,
        CASE WHEN $2='deleted' THEN grove_time.transaction_now() END FROM unnest($1::uuid[]) id",
    )
    .bind(ids)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO locations(file_id,storage_id,object_key)
        SELECT id,'s',id::text FROM unnest($1::uuid[]) id",
    )
    .bind(ids)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO leases(file_id,kind,state,expires_at)
        SELECT id,'write',CASE WHEN $2='pending' THEN 'issued' ELSE 'expired' END,
        grove_time.transaction_now()+interval '1 hour' FROM unnest($1::uuid[]) id",
    )
    .bind(ids)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn real_worker_advances_past_forty_denials_in_each_generic_job(pool: PgPool) {
    time::install_clock(&pool).await;
    let state = crate::routes::tests::test_state();
    let denied = Arc::new(AtomicUsize::new(0));
    let succeeded = Arc::new(AtomicUsize::new(0));
    let repaired = Arc::new(AtomicBool::new(false));
    let failures = denied.clone();
    let successes = succeeded.clone();
    let available = repaired.clone();
    let provider = Router::new().fallback(move |request: axum::extract::Request| {
        let failures = failures.clone();
        let successes = successes.clone();
        let available = available.clone();
        async move {
            let id = Uuid::parse_str(request.uri().path().rsplit('/').next().unwrap()).unwrap();
            if id.as_u128() % 1000 != 41 && !available.load(Ordering::SeqCst) {
                failures.fetch_add(1, Ordering::SeqCst);
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::empty())
                    .unwrap();
            }
            successes.fetch_add(1, Ordering::SeqCst);
            if request.method() == "HEAD" {
                Response::builder()
                    .header("content-length", "10")
                    .header("etag", "\"etag\"")
                    .body(Body::empty())
                    .unwrap()
            } else {
                assert_eq!(request.method(), "DELETE");
                Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap()
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let provider = tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    register(&pool, &state.crypto, &endpoint).await;
    let mut healthy = Vec::new();
    for (index, phase) in ["pending", "reclaimed", "deleted"].into_iter().enumerate() {
        let ids: Vec<_> = (1..=41)
            .map(|id| Uuid::from_u128(index as u128 * 1000 + id))
            .collect();
        healthy.push(*ids.last().unwrap());
        seed(&pool, phase, &ids).await;
    }
    let shutdown = CancellationToken::new();
    let worker = tokio::spawn(run(
        pool.clone(),
        state.crypto.clone(),
        state.s3_clients.clone(),
        Duration::from_millis(10),
        shutdown.clone(),
        Arc::new(grove_core::time::FixedClock(time::base())),
    ));
    let progress = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let complete: bool = sqlx::query_scalar(
                "SELECT
                EXISTS (SELECT 1 FROM files WHERE id=$1 AND state='active')
                AND NOT EXISTS (SELECT 1 FROM locations WHERE file_id=ANY($2))",
            )
            .bind(healthy.first().unwrap())
            .bind(healthy.get(1..).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
            if complete {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(
        progress.is_ok(),
        "healthy candidates did not progress behind the failing batches"
    );
    assert_eq!(denied.load(Ordering::SeqCst), 120);
    assert_eq!(succeeded.load(Ordering::SeqCst), 3);
    time::set_time(&pool, time::base() + chrono::Duration::seconds(1)).await;
    let retained: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM files f JOIN locations l ON l.file_id=f.id
        WHERE f.id<>ALL($1) AND f.recovery_after>grove_time.transaction_now()",
    )
    .bind(&healthy)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, 120);
    assert_eq!(
        files::prune_terminal_files(&pool, 0, 1000).await.unwrap(),
        0
    );
    files::prune_terminal_leases(&pool, 0, 1000).await.unwrap();
    let reclaimed_handles: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM leases le
        JOIN files f ON f.id=le.file_id WHERE f.state='reclaimed' AND f.id<>ALL($1)",
    )
    .bind(&healthy)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reclaimed_handles, 40);

    repaired.store(true, Ordering::SeqCst);
    time::set_time(&pool, time::base() + chrono::Duration::seconds(30)).await;
    let shutdown = CancellationToken::new();
    let worker = tokio::spawn(run(
        pool.clone(),
        state.crypto.clone(),
        state.s3_clients.clone(),
        Duration::from_millis(10),
        shutdown.clone(),
        Arc::new(grove_core::time::FixedClock(time::base())),
    ));
    let recovery = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM files f
                JOIN locations l ON l.file_id=f.id WHERE f.state<>'active'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if remaining == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    provider.abort();
    assert!(
        recovery.is_ok(),
        "persisted intents did not recover after backend repair"
    );
    assert_eq!(denied.load(Ordering::SeqCst), 120);
    assert_eq!(succeeded.load(Ordering::SeqCst), 123);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn shutdown_during_cleanup_preserves_intent_and_releases_the_worker_lock(pool: PgPool) {
    time::install_clock(&pool).await;
    let state = crate::routes::tests::test_state();
    let entered = Arc::new(tokio::sync::Notify::new());
    let started = entered.clone();
    let provider = Router::new().fallback(move |request: axum::extract::Request| {
        let started = started.clone();
        async move {
            assert_eq!(request.method(), "DELETE");
            started.notify_one();
            std::future::pending::<StatusCode>().await
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let provider = tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    register(&pool, &state.crypto, &endpoint).await;
    let file_id = Uuid::new_v4();
    seed(&pool, "reclaimed", &[file_id]).await;
    let shutdown = CancellationToken::new();
    let worker = tokio::spawn(run(
        pool.clone(),
        state.crypto.clone(),
        state.s3_clients.clone(),
        Duration::from_secs(60),
        shutdown.clone(),
        Arc::new(grove_core::time::FixedClock(time::base())),
    ));
    let observed = tokio::time::timeout(Duration::from_secs(5), entered.notified()).await;
    shutdown.cancel();
    let stopped = tokio::time::timeout(Duration::from_secs(2), worker).await;
    provider.abort();
    assert!(observed.is_ok(), "physical cleanup did not start");
    stopped.unwrap().unwrap();
    time::set_time(&pool, time::base() + chrono::Duration::seconds(1)).await;
    let retained: bool = sqlx::query_scalar(
        "SELECT f.state='reclaimed'
        AND f.recovery_after>grove_time.transaction_now()
        AND EXISTS (SELECT 1 FROM locations l WHERE l.file_id=f.id)
        AND EXISTS (SELECT 1 FROM leases le WHERE le.file_id=f.id)
        FROM files f WHERE f.id=$1",
    )
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(retained);
    assert_eq!(files::prune_terminal_leases(&pool, 0, 20).await.unwrap(), 0);
    assert_eq!(files::prune_terminal_files(&pool, 0, 20).await.unwrap(), 0);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if grove_db::with_reconciler_lock(&pool, || async {})
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
