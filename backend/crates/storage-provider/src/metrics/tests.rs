#![allow(clippy::unwrap_used)]
use super::*;
use std::{sync::Arc, time::Duration};

fn render(metrics: &S3Metrics) -> String {
    let mut registry = Registry::with_prefix("grove");
    metrics.register(&mut registry);
    let mut text = String::new();
    prometheus_client::encoding::text::encode(&mut text, &registry).unwrap();
    text
}

#[tokio::test(start_paused = true)]
async fn outcomes_preserve_results_and_never_capture_error_payloads() {
    let metrics = S3Metrics::default();
    let result = metrics
        .observe(Operation::PutObject, async {
            tokio::time::sleep(Duration::from_millis(1250)).await;
            Ok::<_, SdkError<()>>("private-payload")
        })
        .await;
    assert_eq!(result.unwrap(), "private-payload");
    let timeout = metrics
        .observe(Operation::PutObject, async {
            Err::<(), _>(SdkError::<()>::timeout_error(std::io::Error::other(
                "private-secret",
            )))
        })
        .await;
    assert!(matches!(timeout, Err(SdkError::TimeoutError(_))));
    let error = metrics
        .observe(Operation::PutObject, async {
            Err::<(), _>(SdkError::<()>::construction_failure(std::io::Error::other(
                "private-endpoint",
            )))
        })
        .await;
    assert!(matches!(error, Err(SdkError::ConstructionFailure(_))));
    let text = render(&metrics);
    assert!(text.contains("# UNIT grove_provider_call_duration_seconds seconds"));
    for (class, outcome) in [("2xx", "success"), ("none", "timeout"), ("none", "error")] {
        assert!(text.contains(&format!("grove_provider_calls_total{{operation=\"put_object\",status_class=\"{class}\",outcome=\"{outcome}\"}} 1")));
    }
    assert!(
        text.contains("grove_provider_call_duration_seconds_sum{operation=\"put_object\"} 1.25")
    );
    assert!(text.contains("grove_provider_calls_in_flight{operation=\"put_object\"} 0"));
    assert!(!text.contains("private"));
    assert!(!render(&S3Metrics::default()).contains("operation=\"put_object\""));
}

#[tokio::test(start_paused = true)]
async fn cancellation_counts_one_interruption_and_releases_in_flight() {
    let metrics = S3Metrics::default();
    let mut call = Box::pin(metrics.observe(
        Operation::HeadObject,
        std::future::pending::<Result<(), SdkError<()>>>(),
    ));
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(call.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    assert!(
        render(&metrics).contains("grove_provider_calls_in_flight{operation=\"head_object\"} 1")
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    drop(call);
    let text = render(&metrics);
    assert!(text.contains("grove_provider_calls_total{operation=\"head_object\",status_class=\"none\",outcome=\"interrupted\"} 1"));
    assert!(text.contains("grove_provider_calls_in_flight{operation=\"head_object\"} 0"));
    assert!(
        text.contains("grove_provider_call_duration_seconds_sum{operation=\"head_object\"} 2.0")
    );
}

fn spec(endpoint: String) -> crate::S3StorageSpec {
    crate::S3StorageSpec {
        public_endpoint: endpoint.clone(),
        endpoint,
        region: "private-region".into(),
        bucket: "private-bucket".into(),
        force_path_style: true,
        access_key: "private-access".into(),
        secret_key: secrecy::SecretString::from("private-secret".to_owned()),
    }
}

#[tokio::test]
async fn sdk_not_found_denied_and_retries_keep_the_provider_boundary() {
    use axum::{Router, http::StatusCode, routing::any};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    let app = Router::new().fallback(any(move |request: axum::extract::Request| {
        let seen = seen.clone();
        async move {
            if request.uri().path().ends_with("missing") {
                (
                    StatusCode::NOT_FOUND,
                    "<Error><Code>NoSuchKey</Code><Message>private-path</Message></Error>",
                )
            } else if request.uri().path().ends_with("denied") {
                (
                    StatusCode::FORBIDDEN,
                    "<Error><Code>AccessDenied</Code><Message>private-secret</Message></Error>",
                )
            } else {
                seen.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "<Error><Code>InternalError</Code><Message>private-path</Message></Error>",
                )
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let metrics = S3Metrics::default();
    let cache = crate::S3ClientCache::with_metrics(
        Arc::new(grove_core::time::SystemClock),
        metrics.clone(),
    );
    let mut storage = cache.get("private-storage", &spec(endpoint), crate::Address::Internal);
    // A test-only two-attempt policy makes the logical-send boundary explicit.
    storage.client = aws_sdk_s3::Client::from_conf(
        storage
            .client
            .config()
            .to_builder()
            .retry_config(aws_sdk_s3::config::retry::RetryConfig::standard().with_max_attempts(2))
            .build(),
    );
    assert!(
        crate::s3_open_read(&storage, "missing")
            .await
            .unwrap()
            .is_none()
    );
    assert!(crate::s3_delete_object(&storage, "denied").await.is_err());
    assert!(
        tokio::time::timeout(
            Duration::from_secs(10),
            crate::s3_delete_object(&storage, "private-key")
        )
        .await
        .unwrap()
        .is_err()
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let text = render(&metrics);
    assert!(text.contains("grove_provider_calls_total{operation=\"get_object\",status_class=\"4xx\",outcome=\"not_found\"} 1"));
    assert!(text.contains("grove_provider_calls_total{operation=\"delete_object\",status_class=\"4xx\",outcome=\"error\"} 1"));
    assert!(text.contains("grove_provider_calls_total{operation=\"delete_object\",status_class=\"5xx\",outcome=\"error\"} 1"));
    assert!(!text.contains("private"));
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn connector_timeout_is_not_a_generic_transport_error() {
    let metrics = S3Metrics::default();
    let error =
        aws_sdk_s3::error::ConnectorError::timeout(Box::new(std::io::Error::other("private-url")));
    let result = metrics
        .observe(Operation::HeadObject, async {
            Err::<(), _>(SdkError::<()>::dispatch_failure(error))
        })
        .await;
    assert!(matches!(result, Err(SdkError::DispatchFailure(_))));
    let text = render(&metrics);
    assert!(text.contains("grove_provider_calls_total{operation=\"head_object\",status_class=\"none\",outcome=\"timeout\"} 1"));
    assert!(!text.contains("private"));
}

#[tokio::test]
async fn get_finishes_at_headers_while_the_body_is_still_pending() {
    use axum::{Router, body::Body, routing::any};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut writer, reader) = tokio::io::duplex(16);
    let reader = Arc::new(tokio::sync::Mutex::new(Some(reader)));
    let app = Router::new().fallback(any(move || {
        let reader = reader.clone();
        async move {
            let stream = tokio_util::io::ReaderStream::new(reader.lock().await.take().unwrap());
            axum::http::Response::builder()
                .header("content-length", "7")
                .body(Body::from_stream(stream))
                .unwrap()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let metrics = S3Metrics::default();
    let cache = crate::S3ClientCache::with_metrics(
        Arc::new(grove_core::time::SystemClock),
        metrics.clone(),
    );
    let storage = cache.get("private-storage", &spec(endpoint), crate::Address::Internal);
    crate::s3_presign_get(&storage, "private-key", None, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(!render(&metrics).contains("operation=\"get_object\""));
    let (mut body, length) = tokio::time::timeout(
        Duration::from_secs(5),
        crate::s3_open_read(&storage, "private-key"),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(length, 7);
    let at_headers = render(&metrics);
    assert!(at_headers.contains("grove_provider_calls_total{operation=\"get_object\",status_class=\"2xx\",outcome=\"success\"} 1"));
    assert!(at_headers.contains("grove_provider_calls_in_flight{operation=\"get_object\"} 0"));
    let mut bytes = Vec::new();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), body.read_to_end(&mut bytes))
            .await
            .is_err()
    );
    writer.write_all(b"payload").await.unwrap();
    writer.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), body.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bytes, b"payload");
    assert_eq!(render(&metrics), at_headers);
    assert!(!at_headers.contains("private"));
    server.abort();
    let _ = server.await;
}
