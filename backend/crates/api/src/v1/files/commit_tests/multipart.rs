use super::fixture::{ETAG, Fixture};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, Response, StatusCode},
    routing::post,
};
use filegate_db::{PgPool, files};
use grove_object_policy::multipart::composite_etag;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SECOND_ETAG: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Provider {
    endpoint: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Provider {
    async fn start(fail: bool) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let router = Router::new().fallback(post(move |request: Request<Body>| {
            let calls = captured.clone();
            async move {
                to_bytes(request.into_body(), 8192).await.unwrap();
                calls.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if fail {
                    (403, "<Error><Code>AccessDenied</Code></Error>".to_owned())
                } else {
                    (200, format!(
                        "<CompleteMultipartUploadResult><ETag>\"{}\"</ETag></CompleteMultipartUploadResult>",
                        composite_etag([ETAG, SECOND_ETAG])
                    ))
                };
                Response::builder().status(status).header("content-type", "application/xml")
                    .body(Body::from(body)).unwrap()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            calls,
            task,
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn prepare_parts(api: &Fixture) {
    let lease = files::write_lease(&api.state.pool, api.file)
        .await
        .unwrap()
        .unwrap();
    files::attach_upload_id(&api.state.pool, lease.lease_id, "vendor-upload")
        .await
        .unwrap();
    for (number, size, etag) in [(1, 4, ETAG), (2, 3, SECOND_ETAG)] {
        assert!(
            files::record_part_done(&api.state.pool, lease.lease_id, number, size, etag)
                .await
                .unwrap()
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn multipart_validates_parts_before_provider_completion_and_is_idempotent(pool: PgPool) {
    let provider = Provider::start(false).await;
    let api = Fixture::multipart(pool, &provider.endpoint).await;
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"upload is incomplete (missing parts)"})
        )
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    api.assert_pending().await;
    prepare_parts(&api).await;
    for _ in 0..2 {
        assert_eq!(
            api.commit("app").await,
            (
                StatusCode::OK,
                json!({
                    "file_id":api.file,"state":"active","etag":composite_etag([ETAG, SECOND_ETAG])
                })
            )
        );
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let recovery: i64 =
        sqlx::query_scalar("SELECT count(*) FROM native_multipart_completions WHERE file_id=$1")
            .bind(api.file)
            .fetch_one(&api.state.pool)
            .await
            .unwrap();
    assert_eq!(recovery, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_provider_completion_retains_claim_and_does_not_finalize(pool: PgPool) {
    let provider = Provider::start(true).await;
    let api = Fixture::multipart(pool, &provider.endpoint).await;
    prepare_parts(&api).await;
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::BAD_GATEWAY,
            json!({"error":"storage unavailable"})
        )
    );
    api.assert_pending().await;
    let recovery: (String, String) = sqlx::query_as(
        "SELECT state,expected_etag FROM native_multipart_completions WHERE file_id=$1",
    )
    .bind(api.file)
    .fetch_one(&api.state.pool)
    .await
    .unwrap();
    assert_eq!(
        recovery,
        ("completing".into(), composite_etag([ETAG, SECOND_ETAG]))
    );
    assert_eq!(
        api.commit("app").await,
        (
            StatusCode::CONFLICT,
            json!({"error":"multipart completion is already in progress"})
        )
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
