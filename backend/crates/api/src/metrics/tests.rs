#![allow(clippy::unwrap_used)]

use super::*;
use crate::logging::{
    http, transfer,
    worker::{Report, Run},
};
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{Request, StatusCode, header},
    middleware,
    routing::get,
};
use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};
use tower::ServiceExt;

fn labels() -> HttpLabels {
    HttpLabels {
        surface: "s3",
        method: "GET".into(),
        route: "/{bucket}/{*key}".into(),
    }
}

fn observed(router: Router, metrics: &Metrics) -> Router {
    router.layer(middleware::from_fn_with_state(
        metrics.clone(),
        http::observe,
    ))
}

#[tokio::test(start_paused = true)]
async fn requests_are_counted_without_logging_or_sensitive_label_values() {
    let metrics = Metrics::default();
    let app = observed(
        Router::new().route(
            "/{bucket}/{*key}",
            get(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers.get("x-request-id").unwrap(), "signed-private-id");
                tokio::time::advance(Duration::from_millis(25)).await;
                "ok"
            }),
        ),
        &metrics,
    );
    for i in 0..20 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/private-bucket/private-file-{i}?X-Amz-Signature=private-signature"
                    ))
                    .header("x-request-id", "signed-private-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let key = labels();
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&HttpResult {
                request: key.clone(),
                status_class: "2xx",
                outcome: "success"
            })
            .get(),
        20
    );
    assert_eq!(metrics.0.http_active.get_or_create(&key).get(), 0);
    assert_eq!(metrics.0.http_duration.get_or_create(&key).count(), 20);
    assert!((metrics.0.http_duration.get_or_create(&key).sum() - 0.5).abs() < 0.000001);
    let text = metrics.render().unwrap();
    assert!(!text.contains("private"));
    assert!(!text.contains("request_id="));
    assert!(text.contains("route=\"/{bucket}/{*key}\""));
    assert!(text.ends_with("# EOF\n"));
}

#[tokio::test(start_paused = true)]
async fn cancellation_records_one_interruption_and_releases_in_flight() {
    let metrics = Metrics::default();
    let app = observed(
        Router::new().route(
            "/{bucket}/{*key}",
            get(std::future::pending::<&'static str>),
        ),
        &metrics,
    );
    let request = tokio::spawn(
        app.oneshot(
            Request::builder()
                .uri("/bucket/key")
                .body(Body::empty())
                .unwrap(),
        ),
    );
    tokio::task::yield_now().await;
    assert_eq!(metrics.0.http_active.get_or_create(&labels()).get(), 1);
    tokio::time::advance(Duration::from_secs(2)).await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    assert_eq!(metrics.0.http_active.get_or_create(&labels()).get(), 0);
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&HttpResult {
                request: labels(),
                status_class: "none",
                outcome: "interrupted"
            })
            .get(),
        1
    );
    assert_eq!(metrics.0.http_duration.get_or_create(&labels()).sum(), 2.0);
}

#[tokio::test]
async fn status_class_is_preserved_without_counting_unsupported_s3_as_server_failure() {
    let metrics = Metrics::default();
    for (status, class, outcome) in [
        (403, "4xx", "rejected"),
        (404, "4xx", "rejected"),
        (412, "4xx", "rejected"),
        (500, "5xx", "error"),
        (501, "5xx", "rejected"),
    ] {
        let app = observed(
            Router::new().route(
                "/{bucket}/{*key}",
                get(move || async move { StatusCode::from_u16(status).unwrap() }),
            ),
            &metrics,
        );
        app.oneshot(
            Request::builder()
                .uri("/bucket/key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert!(
            metrics
                .0
                .http_requests
                .get_or_create(&HttpResult {
                    request: labels(),
                    status_class: class,
                    outcome
                })
                .get()
                > 0
        );
    }
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&HttpResult {
                request: labels(),
                status_class: "5xx",
                outcome: "error"
            })
            .get(),
        1
    );
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&HttpResult {
                request: labels(),
                status_class: "5xx",
                outcome: "rejected"
            })
            .get(),
        1
    );
}

#[tokio::test]
async fn unmatched_paths_and_methods_cannot_create_unbounded_labels() {
    let metrics = Metrics::default();
    let app = observed(Router::new(), &metrics);
    for i in 0..20 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(format!("PRIVATE{i}").as_str())
                    .uri(format!("/private-key-{i}?token=private-{i}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    let key = HttpLabels {
        surface: "s3",
        method: "_OTHER".into(),
        route: String::new(),
    };
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&HttpResult {
                request: key,
                status_class: "4xx",
                outcome: "rejected"
            })
            .get(),
        20
    );
    assert!(!metrics.render().unwrap().contains("private"));
    assert!(!metrics.render().unwrap().contains("PRIVATE"));
}

#[tokio::test]
async fn transfer_bytes_are_live_and_registries_are_isolated() {
    let metrics = Metrics::default();
    let other = Metrics::default();
    let mut transfer = transfer::Transfer::new("receive", 8, metrics.clone());
    transfer.bytes(4);
    let direction = vec![("direction", "receive")];
    assert_eq!(metrics.0.transfer_bytes.get_or_create(&direction).get(), 4);
    assert_eq!(metrics.0.transfer_active.get_or_create(&direction).get(), 1);
    assert_eq!(
        metrics
            .0
            .transfers
            .get_or_create(&TransferLabels {
                direction: "receive",
                outcome: "success"
            })
            .get(),
        0
    );
    assert_eq!(other.0.transfer_bytes.get_or_create(&direction).get(), 0);
    drop(transfer);
    assert_eq!(metrics.0.transfer_active.get_or_create(&direction).get(), 0);
    assert_eq!(
        metrics
            .0
            .transfers
            .get_or_create(&TransferLabels {
                direction: "receive",
                outcome: "interrupted"
            })
            .get(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn response_and_transfer_measure_different_completion_boundaries() {
    let metrics = Metrics::default();
    let shared = metrics.clone();
    let app = observed(
        Router::new().route(
            "/{bucket}/{*key}",
            get(move || {
                let metrics = shared.clone();
                async move {
                    tokio::time::advance(Duration::from_millis(100)).await;
                    transfer::download(Body::from("done"), 4, uuid::Uuid::new_v4(), metrics)
                }
            }),
        ),
        &metrics,
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/bucket/key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let active = vec![("direction", "send")];
    assert_eq!(metrics.0.http_duration.get_or_create(&labels()).sum(), 0.1);
    assert_eq!(metrics.0.http_active.get_or_create(&labels()).get(), 0);
    assert_eq!(metrics.0.transfer_active.get_or_create(&active).get(), 1);
    let terminal = TransferLabels {
        direction: "send",
        outcome: "success",
    };
    assert_eq!(metrics.0.transfers.get_or_create(&terminal).get(), 0);
    tokio::time::advance(Duration::from_secs(3)).await;
    assert_eq!(to_bytes(response.into_body(), 4).await.unwrap(), "done");
    assert_eq!(metrics.0.transfer_active.get_or_create(&active).get(), 0);
    assert_eq!(metrics.0.transfers.get_or_create(&terminal).get(), 1);
    assert_eq!(metrics.0.transfer_bytes.get_or_create(&active).get(), 4);
    assert_eq!(
        metrics.0.transfer_duration.get_or_create(&terminal).sum(),
        3.0
    );
    assert!(!metrics.render().unwrap().contains("file_id"));
}

#[tokio::test]
async fn send_eof_error_and_drop_count_exactly_once() {
    let metrics = Metrics::default();
    for (body, expected) in [(Body::empty(), 0), (Body::from("short"), 9)] {
        to_bytes(
            transfer::download(body, expected, uuid::Uuid::new_v4(), metrics.clone()),
            32,
        )
        .await
        .unwrap();
    }
    let broken =
        futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other("private-error"))]);
    assert!(
        to_bytes(
            transfer::download(
                Body::from_stream(broken),
                10,
                uuid::Uuid::new_v4(),
                metrics.clone()
            ),
            32
        )
        .await
        .is_err()
    );
    drop(transfer::download(
        Body::empty(),
        10,
        uuid::Uuid::new_v4(),
        metrics.clone(),
    ));
    for outcome in ["success", "incomplete", "error", "interrupted"] {
        assert_eq!(
            metrics
                .0
                .transfers
                .get_or_create(&TransferLabels {
                    direction: "send",
                    outcome
                })
                .get(),
            1
        );
    }
    assert_eq!(
        metrics
            .0
            .transfer_bytes
            .get_or_create(&vec![("direction", "send")])
            .get(),
        5
    );
    assert_eq!(
        metrics
            .0
            .transfer_active
            .get_or_create(&vec![("direction", "send")])
            .get(),
        0
    );
    assert!(!metrics.render().unwrap().contains("private"));
}

#[tokio::test(start_paused = true)]
async fn receive_counts_incomplete_rejected_idle_and_cancelled_streams() {
    let metrics = Metrics::default();
    let path = std::path::Path::new("unused-metrics-spool");
    for (body, expected) in [
        (Body::from("ok"), 2),
        (Body::from("short"), 9),
        (Body::from("large"), 1),
    ] {
        let _ = crate::spool::spool_to_temp(
            body,
            &mut tokio::io::sink(),
            path,
            expected,
            false,
            metrics.clone(),
        )
        .await;
    }
    let pending =
        Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
    assert!(matches!(
        crate::spool::spool_to_temp(
            pending,
            &mut tokio::io::sink(),
            path,
            1,
            false,
            metrics.clone()
        )
        .await,
        Err(crate::spool::SpoolError::Idle)
    ));
    let shared = metrics.clone();
    let task = tokio::spawn(async move {
        let pending =
            Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
        crate::spool::spool_to_temp(pending, &mut tokio::io::sink(), path, 1, false, shared).await
    });
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    for outcome in [
        "success",
        "incomplete",
        "rejected",
        "timeout",
        "interrupted",
    ] {
        assert_eq!(
            metrics
                .0
                .transfers
                .get_or_create(&TransferLabels {
                    direction: "receive",
                    outcome
                })
                .get(),
            1
        );
    }
    assert_eq!(
        metrics
            .0
            .transfer_duration
            .get_or_create(&TransferLabels {
                direction: "receive",
                outcome: "timeout"
            })
            .sum(),
        30.0
    );
    assert_eq!(
        metrics
            .0
            .transfer_active
            .get_or_create(&vec![("direction", "receive")])
            .get(),
        0
    );
}

#[derive(Debug)]
struct TestClock(AtomicI64);
impl Clock for TestClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(self.0.load(Ordering::SeqCst), 0).unwrap()
    }
}

#[tokio::test(start_paused = true)]
async fn worker_success_failure_timeout_skip_and_drop_are_not_conflated() {
    let clock = Arc::new(TestClock(AtomicI64::new(1000)));
    let metrics = Metrics::new(clock.clone(), None);
    let key = WorkerLabels {
        worker: "object",
        task: "purge",
    };
    metrics.register_worker(&key);
    assert_eq!(metrics.0.worker_last_success.get_or_create(&key).get(), 0);
    let mut run = Run::new("object", "purge", metrics.clone());
    tokio::time::advance(Duration::from_secs(3)).await;
    run.finish(&Report {
        processed: 2,
        ..Report::default()
    });
    run.finish(&Report::default());
    drop(run);
    clock.0.store(2000, Ordering::SeqCst);
    metrics.register_worker(&key);
    let mut failed = Run::new("object", "purge", metrics.clone());
    failed.finish(&Report {
        failed: 1,
        ..Report::default()
    });
    let mut timeout = Run::new("object", "purge", metrics.clone());
    timeout.timed_out();
    let mut skipped = Run::new("object", "purge", metrics.clone());
    skipped.skipped();
    drop(Run::new("object", "purge", metrics.clone()));
    for outcome in ["success", "error", "timeout", "skipped", "interrupted"] {
        assert_eq!(
            metrics
                .0
                .worker_runs
                .get_or_create(&WorkerResult {
                    job: key.clone(),
                    outcome
                })
                .get(),
            1
        );
    }
    assert_eq!(
        metrics.0.worker_last_success.get_or_create(&key).get(),
        1000
    );
    assert_eq!(metrics.0.worker_active.get_or_create(&key).get(), 0);
    assert_eq!(metrics.0.worker_processed.get_or_create(&key).get(), 2);
    assert_eq!(metrics.0.worker_failures.get_or_create(&key).get(), 1);
    assert_eq!(metrics.0.worker_skipped.get_or_create(&key).get(), 1);
    assert_eq!(metrics.0.worker_duration.get_or_create(&key).count(), 5);
    assert_eq!(metrics.0.worker_duration.get_or_create(&key).sum(), 3.0);
}

#[tokio::test]
async fn scrape_uses_pool_and_reservation_state_without_database_queries() {
    let metrics = Metrics::default();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(7)
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let budget = Arc::new(crate::spool::SpoolBudget::new(4 * 1024 * 1024));
    let permit = budget.try_acquire(1).unwrap();
    let observations =
        grove_db::observations::pool("postgres://unused:unused@127.0.0.1:1/unused").unwrap();
    let app = endpoint::router(
        metrics.clone(),
        pool.clone(),
        pool.clone(),
        Some(observations.clone()),
        budget,
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/openmetrics-text")
    );
    let text = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(text.contains("# TYPE grove_log_dropped_lines counter"));
    assert!(text.contains(&format!(
        "grove_build_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(text.ends_with("# EOF\n"));
    assert_eq!(pool.size(), 0);
    assert_eq!(observations.size(), 0);
    assert!(text.contains("grove_db_pool_max_connections{pool=\"observations\"} 1"));
    assert_eq!(
        metrics
            .0
            .pool_limit
            .get_or_create(&vec![("pool", "requests")])
            .get(),
        7
    );
    assert_eq!(metrics.0.spool_reserved.get(), 1024 * 1024);
    assert_eq!(metrics.0.spool_active.get(), 1);
    drop(permit);
    app.oneshot(
        Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(metrics.0.spool_reserved.get(), 0);
    assert_eq!(metrics.0.spool_active.get(), 0);
    assert!(
        !metrics
            .render()
            .unwrap()
            .contains("grove_http_requests_total{")
    );
}

#[tokio::test]
async fn application_listener_does_not_expose_metrics_or_reserve_an_s3_bucket() {
    assert!(!crate::routes::RESERVED_TOP_LEVEL.contains(&"metrics"));
    let app = crate::routes::app(crate::routes::tests::test_state(), &[]);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok()),
        Some("application/openmetrics-text; version=1.0.0; charset=utf-8")
    );
    assert!(
        !String::from_utf8_lossy(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .contains("grove_build_info")
    );
}
