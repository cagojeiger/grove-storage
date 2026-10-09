//! Cheap process-local admission before management authentication or database I/O.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

const CONCURRENT_REQUESTS: usize = 16;
const REQUESTS_PER_SECOND: usize = 100;

pub(super) struct Admission {
    slots: Arc<Semaphore>,
    window: Mutex<Window>,
}

struct Window {
    start: Instant,
    admitted: usize,
}

impl Admission {
    pub(super) fn new() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(CONCURRENT_REQUESTS)),
            window: Mutex::new(Window {
                start: Instant::now(),
                admitted: 0,
            }),
        }
    }

    fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        let mut window = self.window.lock().ok()?;
        let now = Instant::now();
        if now.duration_since(window.start) >= Duration::from_secs(1) {
            window.start = now;
            window.admitted = 0;
        }
        if window.admitted >= REQUESTS_PER_SECOND {
            return None;
        }
        window.admitted += 1;
        Some(permit)
    }
}

pub(super) async fn guard(
    State(admission): State<Arc<Admission>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(_permit) = admission.try_acquire() else {
        let mut response = if matches!(
            request.uri().path(),
            "/api/admin/commands/v1" | "/api/admin/console-commands/v1"
        ) {
            crate::commands::failure(
                grove_management_command::CommandError::rejected(
                    grove_management_command::ErrorCode::RateLimited,
                ),
                uuid::Uuid::new_v4(),
            )
        } else {
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::CACHE_CONTROL, "no-store")],
                axum::Json(serde_json::json!({"error": "rate_limited"})),
            )
                .into_response()
        };
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        return response;
    };
    next.run(request).await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn rate_budget_is_shared_and_resets_at_the_boundary() {
        let admission = Admission::new();
        for _ in 0..REQUESTS_PER_SECOND {
            assert!(admission.try_acquire().is_some());
        }
        assert!(admission.try_acquire().is_none());
        tokio::time::advance(Duration::from_millis(999)).await;
        assert!(admission.try_acquire().is_none());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(admission.try_acquire().is_some());
    }

    #[tokio::test]
    async fn requests_never_queue_when_concurrency_is_exhausted() {
        let admission = Admission::new();
        let permits: Vec<_> = (0..CONCURRENT_REQUESTS)
            .map(|_| admission.try_acquire())
            .collect();
        assert!(permits.iter().all(Option::is_some));
        assert!(admission.try_acquire().is_none());
        drop(permits);
        assert!(admission.try_acquire().is_some());
    }

    #[tokio::test]
    async fn rejected_requests_do_not_reach_the_handler() {
        use axum::{Router, body::Body, routing::get};
        use tower::ServiceExt;

        let admission = Arc::new(Admission::new());
        let _permits: Vec<_> = (0..CONCURRENT_REQUESTS)
            .map(|_| admission.try_acquire())
            .collect();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let app = Router::new()
            .route(
                "/",
                get(move || async move {
                    handler_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    StatusCode::OK
                }),
            )
            .layer(axum::middleware::from_fn_with_state(admission, guard));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn command_rejection_preserves_the_wire_contract_before_execution() {
        use axum::{
            Router,
            body::{Body, to_bytes},
            routing::post,
        };
        use tower::ServiceExt;

        for path in ["/api/admin/commands/v1", "/api/admin/console-commands/v1"] {
            let admission = Arc::new(Admission::new());
            let _permits: Vec<_> = (0..CONCURRENT_REQUESTS)
                .map(|_| admission.try_acquire())
                .collect();
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let handler_calls = calls.clone();
            let app = Router::new()
                .route(
                    path,
                    post(move || async move {
                        handler_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        StatusCode::OK
                    }),
                )
                .layer(axum::middleware::from_fn_with_state(admission, guard));
            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(path)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()[header::RETRY_AFTER], "1");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let request_id = response.headers()["x-request-id"]
                .to_str()
                .expect("request ID")
                .to_owned();
            assert!(!uuid::Uuid::parse_str(&request_id).expect("UUID").is_nil());
            let body = to_bytes(response.into_body(), 1024).await.expect("body");
            let body: serde_json::Value = serde_json::from_slice(&body).expect("JSON");
            assert_eq!(
                body,
                serde_json::json!({
                    "protocol": 1,
                    "request_id": request_id,
                    "error": {"code": "rate_limited", "outcome": "not_applied"},
                })
            );
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
    }
}
