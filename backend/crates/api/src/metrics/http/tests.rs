#![allow(clippy::unwrap_used)]
use super::*;
use axum::{
    Router,
    body::to_bytes,
    http::{Request, StatusCode, header},
    middleware,
    routing::get,
};
use futures_util::StreamExt;
use std::time::Duration;
use tower::ServiceExt;

#[test]
fn methods_follow_the_standard_defaults_and_full_case_sensitive_override() {
    let defaults = KnownMethods::default();
    for method in [
        "GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH", "QUERY",
    ] {
        assert_eq!(
            defaults.normalize(&axum::http::Method::from_bytes(method.as_bytes()).unwrap()),
            method
        );
    }
    let configured = KnownMethods::parse("GET, PROPFIND,GET").unwrap();
    assert_eq!(configured.normalize(&axum::http::Method::GET), "GET");
    assert_eq!(configured.normalize(&axum::http::Method::POST), "_OTHER");
    assert_eq!(
        configured.normalize(&axum::http::Method::from_bytes(b"PROPFIND").unwrap()),
        "PROPFIND"
    );
    assert_eq!(
        configured.normalize(&axum::http::Method::from_bytes(b"get").unwrap()),
        "_OTHER"
    );
    assert_eq!(
        KnownMethods::parse("")
            .unwrap()
            .normalize(&axum::http::Method::GET),
        "_OTHER"
    );
    for invalid in ["GET,", "GET,,POST", "private\r\nmethod", "private method"] {
        let error = KnownMethods::parse(invalid).err().unwrap().to_string();
        assert!(error.contains("invalid method"));
        assert!(!error.contains("private"));
    }
}

fn labels() -> HttpLabels {
    HttpLabels {
        surface: "s3",
        method: "GET".into(),
        route: "/{bucket}/{*key}".into(),
    }
}

fn histogram(metrics: &Metrics, status: Option<u16>, error: Option<&str>) -> Histogram {
    let mut key = vec![
        ("http_request_method", "GET".to_owned()),
        ("url_scheme", "http".to_owned()),
        ("http_route", labels().route),
    ];
    if let Some(status) = status {
        key.push(("http_response_status_code", status.to_string()));
    }
    if let Some(error) = error {
        key.push(("error_type", error.to_owned()));
    }
    metrics.0.http_server_duration.get_or_create(&key).clone()
}

use prometheus_client::metrics::histogram::Histogram;

#[tokio::test(start_paused = true)]
async fn request_duration_includes_body_wait_but_response_creation_does_not() {
    let metrics = Metrics::default();
    let app = Router::new()
        .route(
            "/{bucket}/{*key}",
            get(|| async {
                tokio::time::advance(Duration::from_millis(100)).await;
                "done"
            }),
        )
        .layer(middleware::from_fn_with_state(
            metrics.clone(),
            crate::logging::http::observe,
        ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/private-bucket/private-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let full = histogram(&metrics, Some(200), None);
    assert_eq!(full.count(), 0);
    assert_eq!(metrics.0.http_duration.get_or_create(&labels()).sum(), 0.1);
    tokio::time::advance(Duration::from_secs(3)).await;
    assert_eq!(to_bytes(response.into_body(), 4).await.unwrap(), "done");
    assert_eq!(full.count(), 1);
    assert!((full.sum() - 3.1).abs() < 0.000_001);
    let text = metrics.render().unwrap();
    assert!(text.contains("# UNIT http_server_request_duration_seconds seconds"));
    assert!(text.contains("# UNIT grove_http_response_creation_duration_seconds seconds"));
    assert!(text.contains("# UNIT grove_transfer_observed_bytes bytes"));
    assert!(text.contains("le=\"0.075\""));
    assert!(!text.contains("grove_http_request_duration_seconds"));
    assert!(!text.contains("private"));
}

#[tokio::test(start_paused = true)]
async fn errors_drops_and_handler_cancellation_have_one_terminal_sample() {
    let metrics = Metrics::default();
    let response =
        ServerRequest::new(metrics.clone(), labels(), "http", false).response(Response::new(
            Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
                std::io::Error::other("private-error"),
            )])),
        ));
    assert!(to_bytes(response.into_body(), 4).await.is_err());
    assert_eq!(
        histogram(&metrics, Some(200), Some("body_error")).count(),
        1
    );
    let response = ServerRequest::new(metrics.clone(), labels(), "http", false)
        .response(Response::new(Body::from("unread")));
    drop(response);
    assert_eq!(
        histogram(&metrics, Some(200), Some("interrupted")).count(),
        1
    );
    let request = ServerRequest::new(metrics.clone(), labels(), "http", false);
    tokio::time::advance(Duration::from_secs(2)).await;
    drop(request);
    let cancelled = histogram(&metrics, None, Some("interrupted"));
    assert_eq!(cancelled.count(), 1);
    assert_eq!(cancelled.sum(), 2.0);
    assert!(!metrics.render().unwrap().contains("private"));
}

#[tokio::test]
async fn exact_length_finishes_without_an_extra_poll_and_mismatch_is_not_success() {
    let metrics = Metrics::default();
    let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"done"))])
        .chain(futures_util::stream::pending());
    let response = Response::builder()
        .header(header::CONTENT_LENGTH, 4)
        .body(Body::from_stream(stream))
        .unwrap();
    let mut body = ServerRequest::new(metrics.clone(), labels(), "http", false)
        .response(response)
        .into_body();
    let frame = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame.into_data().unwrap(), "done");
    drop(body);
    assert_eq!(histogram(&metrics, Some(200), None).count(), 1);
    assert_eq!(
        histogram(&metrics, Some(200), Some("interrupted")).count(),
        0
    );
    for (content, size) in [("short", 9), ("", 1), ("too long", 1)] {
        let response = Response::builder()
            .header(header::CONTENT_LENGTH, size)
            .body(Body::from(content))
            .unwrap();
        let body = ServerRequest::new(metrics.clone(), labels(), "http", false)
            .response(response)
            .into_body();
        assert_eq!(to_bytes(body, 32).await.unwrap(), content);
    }
    assert_eq!(
        histogram(&metrics, Some(200), Some("body_length_mismatch")).count(),
        3
    );
}

#[tokio::test]
async fn head_and_bodyless_statuses_complete_without_polling_or_changing_headers() {
    let metrics = Metrics::default();
    for (method, status) in [("HEAD", 200), ("GET", 204), ("GET", 304)] {
        let mut key = labels();
        key.method = method.into();
        let response = Response::builder()
            .status(status)
            .header(header::CONTENT_LENGTH, 99)
            .body(Body::empty())
            .unwrap();
        let response =
            ServerRequest::new(metrics.clone(), key, "http", method == "HEAD").response(response);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "99");
        assert_eq!(response.status(), StatusCode::from_u16(status).unwrap());
        drop(response);
    }
    let text = metrics.render().unwrap();
    assert_eq!(
        text.lines()
            .filter(
                |line| line.starts_with("http_server_request_duration_seconds_count{")
                    && line.ends_with(" 1")
            )
            .count(),
        3
    );
    assert!(!text.contains("error_type="));
}

#[tokio::test]
async fn standard_http_errors_include_501_without_changing_grove_rejection_semantics() {
    let metrics = Metrics::default();
    let app = Router::new()
        .route(
            "/{bucket}/{*key}",
            get(|| async { StatusCode::NOT_IMPLEMENTED }),
        )
        .layer(middleware::from_fn_with_state(
            metrics.clone(),
            crate::logging::http::observe,
        ));
    app.oneshot(
        Request::builder()
            .uri("/bucket/key")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(histogram(&metrics, Some(501), Some("501")).count(), 1);
    assert_eq!(
        metrics
            .0
            .http_requests
            .get_or_create(&super::super::HttpResult {
                request: labels(),
                status_class: "5xx",
                outcome: "rejected",
            })
            .get(),
        1
    );
}

#[tokio::test]
async fn head_semantics_survive_method_override_and_forwarded_headers_are_not_trusted() {
    let metrics = Metrics::with_http_methods(
        std::sync::Arc::new(grove_core::time::SystemClock),
        None,
        KnownMethods::parse("GET").unwrap(),
    );
    let app = Router::new()
        .route(
            "/{bucket}/{*key}",
            get(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers.get("x-forwarded-proto").unwrap(), "https");
                Response::builder()
                    .header(header::CONTENT_LENGTH, 4)
                    .body(Body::from("data"))
                    .unwrap()
            }),
        )
        .layer(middleware::from_fn_with_state(
            metrics.clone(),
            crate::logging::http::observe,
        ));
    let response = app
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/bucket/key")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(to_bytes(response.into_body(), 4).await.unwrap().is_empty());
    let text = metrics.render().unwrap();
    assert!(text.contains("http_server_request_duration_seconds_count{http_request_method=\"_OTHER\",url_scheme=\"http\",http_route=\"/{bucket}/{*key}\",http_response_status_code=\"200\"} 1"));
    assert!(!text.contains("error_type="));
    assert!(!text.contains("url_scheme=\"https\""));
}

#[tokio::test]
async fn response_trailers_and_unknown_lengths_survive_observation() {
    struct Frames(std::collections::VecDeque<Frame<Bytes>>);
    impl HttpBody for Frames {
        type Data = Bytes;
        type Error = std::io::Error;
        fn poll_frame(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            Poll::Ready(self.0.pop_front().map(Ok))
        }
        fn is_end_stream(&self) -> bool {
            self.0.is_empty()
        }
    }
    let metrics = Metrics::default();
    let mut trailers = axum::http::HeaderMap::new();
    trailers.insert("x-test-trailer", "kept".parse().unwrap());
    let body = Body::new(Frames(
        [
            Frame::data(Bytes::from_static(b"data")),
            Frame::trailers(trailers),
        ]
        .into(),
    ));
    let response =
        ServerRequest::new(metrics.clone(), labels(), "http", false).response(Response::new(body));
    let mut body = response.into_body();
    assert_eq!(body.size_hint().exact(), None);
    let data = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(data.into_data().unwrap(), "data");
    assert_eq!(histogram(&metrics, Some(200), None).count(), 0);
    let end = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        end.into_trailers().unwrap().get("x-test-trailer").unwrap(),
        "kept"
    );
    drop(body);
    assert_eq!(histogram(&metrics, Some(200), None).count(), 1);
    assert_eq!(
        histogram(&metrics, Some(200), Some("interrupted")).count(),
        0
    );
}

#[tokio::test(start_paused = true)]
async fn exact_length_does_not_hide_later_trailers_or_body_errors() {
    struct Frames(std::collections::VecDeque<Result<Frame<Bytes>, std::io::Error>>);
    impl HttpBody for Frames {
        type Data = Bytes;
        type Error = std::io::Error;
        fn poll_frame(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            Poll::Ready(self.0.pop_front())
        }
        fn is_end_stream(&self) -> bool {
            self.0.is_empty()
        }
    }
    for fails in [false, true] {
        let metrics = Metrics::default();
        let mut trailers = axum::http::HeaderMap::new();
        trailers.insert("x-test-trailer", "kept".parse().unwrap());
        let body = Body::new(Frames(
            [
                Ok(Frame::data(Bytes::from_static(b"data"))),
                if fails {
                    Err(std::io::Error::other("private-error"))
                } else {
                    Ok(Frame::trailers(trailers))
                },
            ]
            .into(),
        ));
        let response = Response::builder()
            .header(header::CONTENT_LENGTH, 4)
            .body(body)
            .unwrap();
        let mut body = ServerRequest::new(metrics.clone(), labels(), "http", false)
            .response(response)
            .into_body();
        let data = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data.into_data().unwrap(), "data");
        assert_eq!(histogram(&metrics, Some(200), None).count(), 0);
        tokio::time::advance(Duration::from_secs(3)).await;
        let end = to_bytes(body, 4).await;
        assert_eq!(end.is_err(), fails);
        let completed = histogram(&metrics, Some(200), fails.then_some("body_error"));
        assert_eq!(completed.count(), 1);
        assert_eq!(completed.sum(), 3.0);
        assert_eq!(
            histogram(&metrics, Some(200), Some("interrupted")).count(),
            0
        );
        if fails {
            assert_eq!(histogram(&metrics, Some(200), None).count(), 0);
        }
    }
}
