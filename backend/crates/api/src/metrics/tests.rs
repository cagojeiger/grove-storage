#![allow(clippy::unwrap_used)]

use super::*;
use axum::body::{Body, to_bytes};
use metrics_exporter_prometheus::PrometheusHandle;

fn recording(test: impl FnOnce(PrometheusHandle)) {
    let recorder = builder().unwrap().build_recorder();
    let handle = recorder.handle();
    metrics::with_local_recorder(&recorder, || test(handle));
}

fn contains(handle: &PrometheusHandle, text: &str) {
    assert!(handle.render().contains(text), "{}", handle.render());
}

#[test]
fn paths_queries_and_methods_never_become_identity_labels() {
    for path in [
        "/secret-client/private%20filename?X-Amz-Signature=secret",
        "/another/key",
    ] {
        let request = Request::builder().uri(path).body(Body::empty()).unwrap();
        assert_eq!(labels(&request), ("s3", "get"));
    }
    let request = Request::builder()
        .uri("/bucket/key?uploads")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    assert_eq!(labels(&request), ("s3", "multipart_create"));
}

#[test]
fn scrape_is_in_memory_and_needs_no_management_credentials() {
    use tower::ServiceExt;
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut request = RequestObservation::new("s3", "get");
                request.finish("2xx");
                let response = router(handle)
                    .oneshot(
                        Request::builder()
                            .uri("/metrics")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), 200);
                assert_eq!(response.headers()["cache-control"], "no-store");
                assert!(
                    response.headers()["content-type"]
                        .to_str()
                        .unwrap()
                        .starts_with("text/plain; version=0.0.4")
                );
                let text = String::from_utf8(
                    to_bytes(response.into_body(), 65536)
                        .await
                        .unwrap()
                        .to_vec(),
                )
                .unwrap();
                assert!(text.contains("grove_http_requests_total"));
                assert!(!text.contains("secret-client"));
            });
    });
}

#[test]
fn slot_rejection_and_receive_failure_are_not_transfer_success() {
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let spool = crate::spool::SpoolBudget::new(1024 * 1024 * 1024);
                let permits: Vec<_> = (0..crate::spool::SPOOL_CONCURRENCY_LIMIT)
                    .map(|_| spool.try_acquire(1).unwrap())
                    .collect();
                assert!(spool.try_acquire(1).is_none());
                drop(permits);
                let source = futures_util::stream::iter([Err::<axum::body::Bytes, _>(
                    std::io::Error::other("private-error"),
                )]);
                let path =
                    std::env::temp_dir().join(format!("grove-metrics-{}", uuid::Uuid::new_v4()));
                assert!(
                    crate::spool::spool_to_temp(
                        Body::from_stream(source),
                        &mut tokio::io::sink(),
                        &path,
                        4,
                        false
                    )
                    .await
                    .is_err()
                );
            });
        contains(&handle, "grove_spool_rejections_total{reason=\"slots\"} 1");
        contains(
            &handle,
            "grove_transfers_total{direction=\"receive\",outcome=\"error\"} 1",
        );
        contains(&handle, "grove_transfer_in_flight{direction=\"receive\"} 0");
        assert!(!handle.render().contains("private-error"));
        assert!(!handle.render().contains("outcome=\"success\""));
    });
}

#[test]
fn empty_and_complete_downloads_count_once() {
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                assert!(
                    to_bytes(download_body(Body::empty(), 0), 32)
                        .await
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    to_bytes(download_body(Body::from("data"), 4), 32)
                        .await
                        .unwrap(),
                    "data"
                );
            });
        contains(
            &handle,
            "grove_transfers_total{direction=\"send\",outcome=\"success\"} 2",
        );
        contains(&handle, "grove_transfer_bytes_total{direction=\"send\"} 4");
        contains(&handle, "grove_transfer_in_flight{direction=\"send\"} 0");
        assert!(!handle.render().contains("outcome=\"interrupted\""));
    });
}

#[test]
fn incomplete_error_and_dropped_streams_are_not_success() {
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let body = download_body(Body::from("short"), 10);
                assert_eq!(to_bytes(body, 32).await.unwrap().len(), 5);
                let source = futures_util::stream::iter([Err::<axum::body::Bytes, _>(
                    std::io::Error::other("broken"),
                )]);
                assert!(
                    to_bytes(download_body(Body::from_stream(source), 10), 32)
                        .await
                        .is_err()
                );
                drop(download_body(Body::from("unread"), 6));
            });
        for outcome in ["incomplete", "error", "interrupted"] {
            contains(
                &handle,
                &format!("grove_transfers_total{{direction=\"send\",outcome=\"{outcome}\"}} 1"),
            );
        }
        assert!(!handle.render().contains("outcome=\"success\""));
        contains(&handle, "grove_transfer_in_flight{direction=\"send\"} 0");
    });
}

#[test]
fn content_length_completion_does_not_require_an_extra_eos_poll() {
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                use http_body::Body as _;
                let source = futures_util::stream::iter([Ok::<_, std::io::Error>(
                    axum::body::Bytes::from_static(b"data"),
                )]);
                let mut body = download_body(Body::from_stream(source), 4);
                let frame = futures_util::future::poll_fn(|cx| {
                    std::pin::Pin::new(&mut body).poll_frame(cx)
                })
                .await;
                assert_eq!(frame.unwrap().unwrap().data_ref().unwrap().len(), 4);
                drop(body);
            });
        contains(
            &handle,
            "grove_transfers_total{direction=\"send\",outcome=\"success\"} 1",
        );
        assert!(!handle.render().contains("outcome=\"interrupted\""));
    });
}

#[test]
fn elapsed_metrics_follow_tokio_virtual_time() {
    recording(|handle| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(async {
                let mut transfer = Transfer::new("receive");
                tokio::time::advance(std::time::Duration::from_secs(9)).await;
                transfer.finish("success");
            });
        contains(
            &handle,
            "grove_transfer_duration_seconds_sum{direction=\"receive\",outcome=\"success\"} 9",
        );
    });
}

#[test]
fn cancelled_request_releases_its_gauge() {
    recording(|handle| {
        drop(RequestObservation::new("s3", "put"));
        contains(
            &handle,
            "grove_http_requests_total{surface=\"s3\",operation=\"put\",outcome=\"cancelled\"} 1",
        );
        contains(&handle, "grove_http_in_flight{surface=\"s3\"} 0");
    });
}

#[test]
fn failed_jobs_never_advance_last_success_and_cancellation_is_visible() {
    recording(|handle| {
        let mut job = JobObservation::new("object", "purge");
        job.finish(&JobReport::default(), 100.0);
        drop(job);
        let mut job = JobObservation::new("object", "purge");
        job.finish(
            &JobReport {
                failed: 1,
                ..JobReport::default()
            },
            200.0,
        );
        drop(job);
        drop(JobObservation::new("object", "purge"));
        contains(
            &handle,
            "grove_job_last_success_timestamp_seconds{worker=\"object\",task=\"purge\"} 100",
        );
        contains(
            &handle,
            "grove_job_last_completed_timestamp_seconds{worker=\"object\",task=\"purge\"} 200",
        );
        contains(
            &handle,
            "grove_jobs_total{worker=\"object\",task=\"purge\",outcome=\"interrupted\"} 1",
        );
        contains(
            &handle,
            "grove_job_in_flight{worker=\"object\",task=\"purge\"} 0",
        );
    });
}

#[test]
fn failed_snapshots_keep_the_previous_timestamp() {
    recording(|handle| {
        snapshot::snapshot_result("db", true, 100.0);
        snapshot::snapshot_result("db", false, 0.0);
        contains(&handle, "grove_snapshot_available{source=\"db\"} 0");
        contains(
            &handle,
            "grove_snapshot_last_success_timestamp_seconds{source=\"db\"} 100",
        );
        contains(&handle, "grove_snapshot_errors_total{source=\"db\"} 1");
    });
}

#[tokio::test]
async fn disk_snapshot_counts_regular_files_and_reports_missing_directory() {
    let root = std::env::temp_dir().join(format!("grove-metrics-disk-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&root).await.unwrap();
    tokio::fs::write(root.join("one"), b"123").await.unwrap();
    tokio::fs::write(root.join("two"), b"45678").await.unwrap();
    tokio::fs::create_dir(root.join("directory")).await.unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("one"), root.join("link")).unwrap();
    assert_eq!(snapshot::disk_usage(&root).await.unwrap(), (8, 2));
    tokio::fs::remove_dir_all(&root).await.unwrap();
    assert!(snapshot::disk_usage(&root).await.is_err());
}

#[test]
fn spool_reservations_are_released_and_rejections_are_distinct() {
    recording(|handle| {
        let spool = crate::spool::SpoolBudget::new(1024 * 1024);
        let permit = spool.try_acquire(1).unwrap();
        assert!(spool.try_acquire(1).is_none());
        spool.observe();
        contains(&handle, "grove_spool_reserved_bytes 1048576");
        contains(&handle, "grove_spool_rejections_total{reason=\"bytes\"} 1");
        drop(permit);
        spool.observe();
        contains(&handle, "grove_spool_reserved_bytes 0");
    });
}
