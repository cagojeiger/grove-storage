#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{HeaderMap, Request},
    routing::get,
};
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;
use tracing::Instrument;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("capture lock poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Capture {
    fn records(&self) -> Vec<Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
fn record(mode: LogFormat, level: &str, test: impl FnOnce(Capture)) {
    let capture = Capture::default();
    let writer = capture.clone();
    let clock = grove_core::time::FixedClock(
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .into(),
    );
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new(level))
        .with(filter_fn(payload_safe))
        .with(
            tracing_subscriber::fmt::layer()
                .fmt_fields(format::SafeFields)
                .event_format(format::SafeEvent {
                    mode,
                    clock: Arc::new(clock),
                    errors: None,
                })
                .with_writer(move || writer.clone()),
        );
    tracing::subscriber::with_default(subscriber, || test(capture));
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn json_is_flat_bounded_and_excludes_payloads_in_events_and_spans() {
    record(LogFormat::Json, "trace", |capture| {
        let id = uuid::Uuid::new_v4();
        let span =
            tracing::info_span!("request", request_id=%id, key="secret-key", token="secret-token");
        let _entered = span.enter();
        tracing::error!(
            event = "http.response",
            status = 503_u64,
            error = "secret-error",
            payload = "secret-payload",
            url = "secret-url",
            username = "secret-user",
            "secret-message"
        );
        tracing::trace!(target:"rmcp::service", payload="secret-rmcp");
        tracing::warn!(target:"aws_smithy_http", response="secret-provider");
        let rows = capture.records();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["request_id"], id.to_string());
        assert_eq!(rows[0]["timestamp"], "2026-01-01T00:00:00.000Z");
        assert_eq!(rows[0]["schema_version"], 1);
        assert_eq!(rows[0]["status"], 503);
        assert!(!rows[0].to_string().contains("secret"));
        assert!(rows[0].get("fields").is_none());
    });
}

#[test]
fn pretty_output_uses_the_same_privacy_policy() {
    record(LogFormat::Pretty, "trace", |capture| {
        tracing::debug!(
            event = "runtime.test",
            key = "sentinel-private-key",
            error = "sentinel-error",
            "sentinel-message"
        );
        let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
        assert!(text.starts_with("DEBUG "));
        assert!(!text.contains("sentinel"));
    });
}

#[test]
fn context_updates_are_safe_and_survive_an_error_only_filter() {
    record(LogFormat::Json, "error", |capture| {
        let id = uuid::Uuid::new_v4();
        let span = tracing::error_span!(
            "context",
            request_id = tracing::field::Empty,
            payload = "sentinel"
        );
        span.record("request_id", tracing::field::display(id));
        let _entered = span.enter();
        tracing::error!(event = "history.write_failed", stream = "security_events");
        assert_eq!(capture.records()[0]["request_id"], id.to_string());
        assert!(!capture.records()[0].to_string().contains("sentinel"));
    });
}

#[test]
fn http_keeps_signed_headers_and_omits_raw_paths_and_queries() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            let business_id = uuid::Uuid::new_v4();
            let app = Router::new()
                .route(
                    "/{bucket}/{*key}",
                    get(move |headers: HeaderMap| async move {
                        assert_eq!(headers["x-request-id"], "signed-sentinel");
                        ([("x-request-id", business_id.to_string())], "ok")
                    }),
                )
                .layer(axum::middleware::from_fn(http::observe));
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/sentinel-bucket/sentinel-file?X-Amz-Signature=sentinel-signature")
                        .header("x-request-id", "signed-sentinel")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let rows = capture.records();
            let end = rows.iter().find(|r| r["event"] == "http.response").unwrap();
            assert_eq!(end["operation_id"], business_id.to_string());
            assert!(uuid::Uuid::parse_str(end["request_id"].as_str().unwrap()).is_ok());
            assert_ne!(end["request_id"], end["operation_id"]);
            assert_eq!(end["route"], "/{bucket}/{*key}");
            assert!(!end.to_string().contains("sentinel"));
        })
    });
}

#[test]
fn complete_empty_partial_error_and_dropped_streams_finish_once() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            let id = uuid::Uuid::new_v4();
            let file_id = uuid::Uuid::new_v4();
            let span = tracing::error_span!("request", request_id=%id);
            async {
                for (body, size) in [
                    (Body::empty(), 0),
                    (Body::from("done"), 4),
                    (Body::from("short"), 9),
                ] {
                    let _ = to_bytes(transfer::download(body, size, file_id), 32)
                        .await
                        .unwrap();
                }
                let broken = futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other(
                    "sentinel-error",
                ))]);
                assert!(
                    to_bytes(
                        transfer::download(Body::from_stream(broken), 10, file_id),
                        32
                    )
                    .await
                    .is_err()
                );
                drop(transfer::download(Body::empty(), 10, file_id));
            }
            .instrument(span)
            .await;
            let rows = capture.records();
            assert_eq!(rows.len(), 5);
            for (row, expected) in
                rows.iter()
                    .zip(["success", "success", "incomplete", "error", "interrupted"])
            {
                assert_eq!(row["event"], "transfer.finished");
                assert_eq!(row["outcome"], expected);
                assert_eq!(row["request_id"], id.to_string());
                assert_eq!(row["file_id"], file_id.to_string());
            }
            assert!(!rows.iter().any(|r| r.to_string().contains("sentinel")));
        })
    });
}

#[test]
fn response_creation_and_body_completion_have_distinct_timing_and_shared_identity() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            tokio::time::pause();
            let file_id = uuid::Uuid::new_v4();
            let operation_id = uuid::Uuid::new_v4();
            let app = Router::new()
                .route(
                    "/{bucket}/{*key}",
                    get(move || async move {
                        tokio::time::advance(std::time::Duration::from_millis(100)).await;
                        (
                            [("x-request-id", operation_id.to_string())],
                            transfer::download(Body::from("done"), 4, file_id),
                        )
                    }),
                )
                .layer(axum::middleware::from_fn(http::observe));
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/private-bucket/private-key")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let rows = capture.records();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["event"], "http.response");
            assert_eq!(rows[0]["outcome"], "success");
            assert_eq!(rows[0]["duration_ms"], 100);
            assert_eq!(rows[0]["operation_id"], operation_id.to_string());

            tokio::time::advance(std::time::Duration::from_secs(2)).await;
            assert_eq!(to_bytes(response.into_body(), 4).await.unwrap(), "done");
            let completed = capture.records();
            assert_eq!(completed.len(), 2);
            assert_eq!(completed[1]["event"], "transfer.finished");
            assert_eq!(completed[1]["outcome"], "success");
            assert_eq!(completed[1]["duration_ms"], 2000);
            assert_eq!(completed[1]["observed_bytes"], 4);
            assert_eq!(completed[1]["request_id"], rows[0]["request_id"]);
            assert_eq!(completed[1]["file_id"], file_id.to_string());
            assert!(
                !completed
                    .iter()
                    .any(|row| row.to_string().contains("private"))
            );
        })
    });
}

#[test]
fn download_identity_survives_request_context_and_an_error_only_filter() {
    record(LogFormat::Json, "error", |capture| {
        runtime().block_on(async {
            let request_id = uuid::Uuid::new_v4();
            let file_id = uuid::Uuid::new_v4();
            let body = {
                let request = tracing::error_span!("request", %request_id, key = "private-key");
                let _entered = request.enter();
                let broken = futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other(
                    "private-provider-error",
                ))]);
                transfer::download(Body::from_stream(broken), 10, file_id)
            };
            assert!(to_bytes(body, 32).await.is_err());
            let rows = capture.records();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["event"], "transfer.finished");
            assert_eq!(rows[0]["request_id"], request_id.to_string());
            assert_eq!(rows[0]["file_id"], file_id.to_string());
            assert_eq!(rows[0]["outcome"], "error");
            assert!(!rows[0].to_string().contains("private"));
        })
    });
}

#[test]
fn content_length_completion_does_not_require_a_final_poll() {
    use http_body::Body as _;
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            let stream =
                futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"done"))])
                    .chain(futures_util::stream::pending());
            let mut body = transfer::download(Body::from_stream(stream), 4, uuid::Uuid::new_v4());
            let frame =
                std::future::poll_fn(|cx| std::pin::Pin::new(&mut body).poll_frame(cx)).await;
            assert!(frame.is_some());
            drop(body);
            assert_eq!(capture.records().len(), 1);
            assert_eq!(capture.records()[0]["outcome"], "success");
        })
    });
}

#[test]
fn elapsed_time_and_worker_results_are_virtualizable() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            tokio::time::pause();
            let mut run = worker::Run::new("object", "purge");
            let mut transfer = transfer::Transfer::new("receive", 4);
            tokio::time::advance(std::time::Duration::from_secs(3)).await;
            transfer.bytes(4);
            transfer.finish("success", "");
            run.finish(&worker::Report {
                failed: 1,
                ..worker::Report::default()
            });
            drop(run);
            let abandoned = worker::Run::new("object", "cleanup");
            drop(abandoned);
            let rows = capture.records();
            assert_eq!(rows.len(), 3);
            assert_eq!(rows[0]["duration_ms"], 3000);
            assert_eq!(rows[1]["duration_ms"], 3000);
            assert_eq!(rows[1]["outcome"], "error");
            assert_eq!(rows[1]["failed"], 1);
            assert_eq!(rows[2]["outcome"], "interrupted");
            assert!(rows[1].get("run_id").is_some());
        })
    });
}

#[test]
fn http_levels_distinguish_expected_s3_rejections_and_service_errors() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            let app = Router::new()
                .route(
                    "/{bucket}/{*key}",
                    axum::routing::any(|request: Request<Body>| async move {
                        match request.uri().path() {
                            "/bucket/missing" => axum::http::StatusCode::NOT_FOUND,
                            "/bucket/precondition" => axum::http::StatusCode::PRECONDITION_FAILED,
                            "/bucket/unsupported" => axum::http::StatusCode::NOT_IMPLEMENTED,
                            "/bucket/failed" => axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            _ => axum::http::StatusCode::OK,
                        }
                    }),
                )
                .layer(axum::middleware::from_fn(http::observe));
            for (method, key) in [
                ("GET", "ok"),
                ("GET", "missing"),
                ("PUT", "precondition"),
                ("POST", "unsupported"),
                ("GET", "failed"),
                ("PUT", "ok"),
            ] {
                app.clone()
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri(format!("/bucket/{key}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
            }
            let rows = capture.records();
            assert_eq!(rows.len(), 6);
            for (row, level) in rows
                .iter()
                .zip(["debug", "debug", "debug", "warn", "error", "debug"])
            {
                assert_eq!(row["level"], level);
            }
            assert_eq!(rows[3]["outcome"], "rejected");
            assert_eq!(rows[4]["outcome"], "error");
        })
    });
}

#[test]
fn interrupted_requests_and_timed_out_workers_are_not_successes() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            tokio::time::pause();
            let app = Router::new()
                .route("/waiting", get(std::future::pending::<()>))
                .layer(axum::middleware::from_fn(http::observe));
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    app.oneshot(
                        Request::builder()
                            .uri("/waiting")
                            .body(Body::empty())
                            .unwrap()
                    )
                )
                .await
                .is_err()
            );
            let mut run = worker::Run::new("management", "retention");
            run.timed_out();
            run.finish(&worker::Report::default());
            drop(run);
            let rows = capture.records();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0]["event"], "http.interrupted");
            // Tokio timers have millisecond resolution, including paused auto-advance.
            assert!((2000..=2001).contains(&rows[0]["duration_ms"].as_u64().unwrap()));
            assert_eq!(rows[1]["outcome"], "timeout");
            assert_eq!(rows[1]["error_code"], "pass_timeout");
        })
    });
}

#[test]
fn receive_logs_measure_bytes_and_classify_partial_aborted_and_idle_streams() {
    record(LogFormat::Json, "trace", |capture| {
        runtime().block_on(async {
            tokio::time::pause();
            let path = std::path::Path::new("unused-logging-spool");
            for (body, size) in [(Body::from("done"), 4), (Body::from("short"), 9)] {
                assert!(
                    crate::spool::spool_to_temp(body, &mut tokio::io::sink(), path, size, false)
                        .await
                        .is_ok()
                );
            }
            let broken = futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other(
                "private-body-error",
            ))]);
            assert!(
                crate::spool::spool_to_temp(
                    Body::from_stream(broken),
                    &mut tokio::io::sink(),
                    path,
                    4,
                    false
                )
                .await
                .is_err()
            );
            let pending = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
            assert!(
                crate::spool::spool_to_temp(
                    Body::from_stream(pending),
                    &mut tokio::io::sink(),
                    path,
                    1,
                    false
                )
                .await
                .is_err()
            );
            let rows = capture.records();
            assert_eq!(rows.len(), 4);
            for (row, outcome) in rows
                .iter()
                .zip(["success", "incomplete", "error", "timeout"])
            {
                assert_eq!(row["direction"], "receive");
                assert_eq!(row["outcome"], outcome);
            }
            assert_eq!(rows[1]["observed_bytes"], 5);
            assert_eq!(rows[2]["error_code"], "receive_aborted");
            assert_eq!(rows[3]["error_code"], "receive_idle");
            assert!((30000..=30001).contains(&rows[3]["duration_ms"].as_u64().unwrap()));
            assert!(!rows.iter().any(|r| r.to_string().contains("private")));
        })
    });
}

#[test]
fn bounded_writer_drops_without_backpressure_and_guard_drains_accepted_lines() {
    struct PausedWriter {
        capture: Capture,
        started: Option<std::sync::mpsc::SyncSender<()>>,
        release: std::sync::mpsc::Receiver<()>,
    }
    impl Write for PausedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(started) = self.started.take() {
                started.send(()).map_err(std::io::Error::other)?;
                self.release
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .map_err(std::io::Error::other)?;
            }
            self.capture.write(bytes)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let capture = Capture::default();
    let (started, waiting) = std::sync::mpsc::sync_channel(1);
    let (release, blocker) = std::sync::mpsc::sync_channel(1);
    let (mut writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(1)
        .lossy(true)
        .finish(PausedWriter {
            capture: capture.clone(),
            started: Some(started),
            release: blocker,
        });
    let counter = writer.error_counter();
    writer.write_all(b"first\n").unwrap();
    waiting
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    writer.write_all(b"second\n").unwrap();
    writer.write_all(b"dropped\n").unwrap();
    assert_eq!(counter.dropped_lines(), 1);
    release.send(()).unwrap();
    drop(guard);
    assert_eq!(&*capture.0.lock().unwrap(), b"first\nsecond\n");
}
