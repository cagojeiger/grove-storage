#![allow(clippy::unwrap_used, clippy::panic)]
mod fixture;
mod multipart;
use axum::{Router, body::Body, http::StatusCode, routing::head};
use fixture::{ETAG, Fixture};
use grove_db::{PgPool, files};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU16, AtomicUsize, Ordering},
};

#[sqlx::test(migrations = "../db/migrations")]
async fn lost_transition_response_preserves_ownership_and_current_state(pool: PgPool) {
    use axum::response::IntoResponse;
    let api = Fixture::new(pool, true, "http://unused.invalid").await;
    for (client, expected) in [
        ("app", StatusCode::CONFLICT),
        ("other", StatusCode::NOT_FOUND),
    ] {
        let response = super::committed_or_conflict(
            &api.state,
            &crate::native::ClientId(client.into()),
            api.file,
        )
        .await
        .unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), expected);
    }
    files::finalize_commit(&api.state.pool, api.file, "winning-etag")
        .await
        .unwrap();
    let response =
        super::committed_or_conflict(&api.state, &crate::native::ClientId("app".into()), api.file)
            .await
            .unwrap_or_else(IntoResponse::into_response);
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        json!({
            "file_id":api.file,"state":"active","etag":"winning-etag"
        })
    );
    files::mark_deleted(&api.state.pool, "app", api.file)
        .await
        .unwrap();
    let response =
        super::committed_or_conflict(&api.state, &crate::native::ClientId("app".into()), api.file)
            .await
            .unwrap_or_else(IntoResponse::into_response);
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn relay_commit_preserves_validation_ownership_and_idempotence(pool: PgPool) {
    let api = Fixture::new(pool, true, "http://unused.invalid").await;
    assert_eq!(
        api.commit("other").await,
        (StatusCode::NOT_FOUND, json!({"error":"file not found"}))
    );
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"no uploaded object to commit"})
        )
    );
    api.assert_pending().await;
    api.recorded(6, ETAG).await;
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"uploaded size does not match declaration"})
        )
    );
    api.assert_pending().await;
    api.recorded(7, &"b".repeat(32)).await;
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"uploaded content does not match declared md5"})
        )
    );
    api.assert_pending().await;
    api.recorded(7, &ETAG.to_uppercase()).await;
    for _ in 0..2 {
        assert_eq!(
            api.commit("app").await,
            (
                StatusCode::OK,
                json!({
                    "file_id":api.file,"state":"active","etag":ETAG.to_uppercase()
                })
            )
        );
    }
    let lease: String =
        sqlx::query_scalar("SELECT state FROM leases WHERE file_id=$1 AND kind='write'")
            .bind(api.file)
            .fetch_one(&api.state.pool)
            .await
            .unwrap();
    assert_eq!(lease, "committed");
    files::mark_deleted(&api.state.pool, "app", api.file)
        .await
        .unwrap();
    assert_eq!(
        api.commit("app").await,
        (StatusCode::CONFLICT, json!({"error":"file is deleted"}))
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn direct_commit_distinguishes_provider_failure_from_absence(pool: PgPool) {
    let status = Arc::new(AtomicU16::new(404));
    let count = Arc::new(AtomicUsize::new(0));
    let router = Router::new().fallback(head({
        let status = status.clone();
        let count = count.clone();
        move || {
            count.fetch_add(1, Ordering::SeqCst);
            let status = status.load(Ordering::SeqCst);
            async move {
                axum::http::Response::builder()
                    .status(status)
                    .header("content-length", "7")
                    .header("etag", format!("\"{ETAG}\""))
                    .body(Body::empty())
                    .unwrap()
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let api = Fixture::new(pool, false, &endpoint).await;
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"no uploaded object to commit"})
        )
    );
    api.assert_pending().await;
    status.store(403, Ordering::SeqCst);
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_GATEWAY,
            json!({"error":"storage unavailable"})
        )
    );
    api.assert_pending().await;
    status.store(200, Ordering::SeqCst);
    for _ in 0..2 {
        assert_eq!(
            api.commit("app").await,
            (
                StatusCode::OK,
                json!({"file_id":api.file,"state":"active","etag":ETAG})
            )
        );
    }
    assert_eq!(count.load(Ordering::SeqCst), 3);
    task.abort();
}
