use super::*;
use crate::logging::http::S3ErrorCode;
use axum::response::{IntoResponse, Response};
use http_body::Body as _;

fn finished(capture: &Capture) -> Vec<Value> {
    capture
        .records()
        .into_iter()
        .filter(|row| row["event_name"] == "grove.http.request_finished")
        .collect()
}

#[test]
fn default_info_records_service_completions_but_not_probes_or_console_assets() {
    record(LogFormat::Json, "info", |capture| {
        runtime().block_on(async {
            let app = Router::new()
                .route("/{bucket}/{*key}", axum::routing::any(|| async { "ok" }))
                .route("/healthz", get(|| async { "ok" }))
                .route("/readyz", get(|| async { "ok" }))
                .route("/", get(|| async { "ok" }))
                .route("/api/admin/console", get(|| async { "ok" }))
                .route("/api/admin/console/{*asset}", get(|| async { "ok" }))
                .layer(axum::middleware::from_fn_with_state(
                    crate::metrics::Metrics::default(),
                    http::observe,
                ));
            for path in [
                "/app/private-key",
                "/healthz",
                "/readyz",
                "/",
                "/api/admin/console",
                "/api/admin/console/assets/private.js",
            ] {
                let response = app
                    .clone()
                    .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(to_bytes(response.into_body(), 32).await.unwrap(), "ok");
            }
            let rows = finished(&capture);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["severity_text"], "INFO");
            assert_eq!(rows[0]["body"], "HTTP request finished");
            assert_eq!(rows[0]["attributes"]["grove.operation"], "unknown");
            assert_eq!(rows[0]["attributes"]["grove.outcome"], "success");
            assert!(!rows[0].to_string().contains("private"));
        });
    });
}

#[test]
fn body_errors_drops_and_exact_lengths_emit_one_terminal_event_each() {
    record(LogFormat::Json, "info", |capture| {
        runtime().block_on(async {
            let metrics = crate::metrics::Metrics::default();
            let response = |body: Body, length: u64| {
                let span = tracing::error_span!("request", request_id = %uuid::Uuid::new_v4());
                let labels = crate::metrics::HttpLabels {
                    surface: "s3",
                    method: "GET".into(),
                    route: "/{bucket}/{*key}".into(),
                };
                let mut observer = crate::metrics::http::ServerRequest::new(
                    metrics.clone(),
                    labels,
                    "http",
                    false,
                );
                observer.log_to(span);
                let mut response = body.into_response();
                response
                    .headers_mut()
                    .insert("content-length", length.into());
                observer.response(response)
            };
            drop(response(Body::empty(), 0));
            let stream =
                futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"done"))])
                    .chain(futures_util::stream::pending());
            let mut body = response(Body::from_stream(stream), 4).into_body();
            assert!(
                std::future::poll_fn(|cx| std::pin::Pin::new(&mut body).poll_frame(cx))
                    .await
                    .is_some()
            );
            drop(body);
            to_bytes(response(Body::from("short"), 9).into_body(), 32)
                .await
                .unwrap();
            let broken = futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other(
                "private-error",
            ))]);
            assert!(
                to_bytes(response(Body::from_stream(broken), 4).into_body(), 32)
                    .await
                    .is_err()
            );
            drop(response(
                Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>()),
                4,
            ));

            let rows = finished(&capture);
            assert_eq!(rows.len(), 5);
            for (row, (outcome, code)) in rows.iter().zip([
                ("success", ""),
                ("success", ""),
                ("error", "body_length_mismatch"),
                ("error", "body_error"),
                ("interrupted", "interrupted"),
            ]) {
                assert_eq!(row["attributes"]["grove.outcome"], outcome);
                assert_eq!(row["attributes"]["grove.error_code"], code);
                assert_eq!(row["attributes"]["http.response.status_code"], 200);
            }
            assert!(!format!("{rows:?}").contains("private"));
        });
    });
}

#[test]
fn protocol_error_survives_body_failure_without_logging_the_response() {
    record(LogFormat::Json, "info", |capture| {
        runtime().block_on(async {
            let app = Router::new()
                .route(
                    "/{bucket}/{*key}",
                    get(|| async {
                        let broken = futures_util::stream::iter([Err::<Bytes, _>(
                            std::io::Error::other("private-error"),
                        )]);
                        let mut response = Response::new(Body::from_stream(broken));
                        *response.status_mut() = axum::http::StatusCode::FORBIDDEN;
                        response
                            .extensions_mut()
                            .insert(S3ErrorCode("SignatureDoesNotMatch"));
                        response
                    }),
                )
                .layer(axum::middleware::from_fn_with_state(
                    crate::metrics::Metrics::default(),
                    http::observe,
                ));
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/app/private-key")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(to_bytes(response.into_body(), 32).await.is_err());
            let rows = finished(&capture);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["attributes"]["grove.outcome"], "error");
            assert_eq!(rows[0]["attributes"]["grove.error_code"], "body_error");
            assert_eq!(
                rows[0]["attributes"]["grove.s3_error_code"],
                "SignatureDoesNotMatch"
            );
            assert!(!rows[0].to_string().contains("private"));
        });
    });
}

#[test]
fn real_s3_authentication_rejection_keeps_unknown_operation_and_xml_contract() {
    record(LogFormat::Json, "info", |capture| {
        runtime().block_on(async {
            let app = crate::routes::app(crate::routes::tests::test_state(), &[]);
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/private-bucket/private-key?X-Amz-Signature=private-signature")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 403);
            assert_eq!(response.headers()["content-type"], "application/xml");
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            assert!(
                std::str::from_utf8(&body)
                    .unwrap()
                    .contains("<Code>AccessDenied</Code>")
            );
            let rows = finished(&capture);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["attributes"]["grove.stage"], "authentication");
            assert_eq!(rows[0]["attributes"]["grove.operation"], "unknown");
            assert_eq!(rows[0]["attributes"]["grove.s3_error_code"], "AccessDenied");
            assert_eq!(rows[0]["attributes"]["grove.outcome"], "rejected");
            assert!(!format!("{:?}", capture.records()).contains("private"));
        });
    });
}

#[test]
fn error_filter_retains_request_identity_and_rejects_unsafe_protocol_fields() {
    record(LogFormat::Json, "error", |capture| {
        runtime().block_on(async {
            let app = Router::new()
                .route(
                    "/{bucket}/{*key}",
                    get(|| async {
                        let mut response =
                            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
                        response
                            .extensions_mut()
                            .insert(S3ErrorCode("unsafe\nprivate-code"));
                        response
                    }),
                )
                .layer(axum::middleware::from_fn_with_state(
                    crate::metrics::Metrics::default(),
                    http::observe,
                ));
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/app/private-key")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            drop(response);
            let rows = finished(&capture);
            assert_eq!(rows.len(), 1);
            assert!(
                uuid::Uuid::parse_str(rows[0]["attributes"]["grove.request_id"].as_str().unwrap())
                    .is_ok()
            );
            assert!(rows[0]["attributes"].get("grove.s3_error_code").is_none());
            assert!(rows[0]["attributes"].get("grove.error_code").is_none());
            assert!(!format!("{:?}", capture.records()).contains("private"));
        });
    });
}
