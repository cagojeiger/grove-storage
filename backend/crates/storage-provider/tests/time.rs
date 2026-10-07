#![allow(clippy::unwrap_used)]
use grove_core::time::FixedClock;
use grove_storage_provider::{
    Address, S3ClientCache, S3StorageSpec, s3_presign_get, s3_presign_put, s3_presign_upload_part,
};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn sdk_network_request_signing_uses_the_injected_clock() {
    use axum::{Router, body::Body, extract::State, http::Request, routing::any};
    use std::sync::Mutex;
    let dates = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new().fallback(any(
        |State(dates): State<Arc<Mutex<Vec<String>>>>, request: Request<Body>| async move {
            dates.lock().unwrap().push(
                request.headers().get("x-amz-date").unwrap().to_str().unwrap().to_owned(),
            );
            let body = if request.method() == "HEAD" {
                ""
            } else {
                "<ListMultipartUploadsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><IsTruncated>false</IsTruncated></ListMultipartUploadsResult>"
            };
            // The SDK also observes provider time for clock-skew correction.
            axum::http::Response::builder()
                .header("date", "Thu, 01 Jan 2026 00:00:00 GMT")
                .body(Body::from(body)).unwrap()
        },
    )).with_state(dates.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let spec = S3StorageSpec {
        endpoint,
        public_endpoint: "https://public.invalid".into(),
        region: "local".into(),
        bucket: "objects".into(),
        force_path_style: true,
        access_key: "access".into(),
        secret_key: secrecy::SecretString::from("secret".to_owned()),
    };
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        grove_storage_provider::s3_connect_with_clock(&spec, Arc::new(FixedClock(now))),
    )
    .await;
    server.abort();
    result.unwrap().unwrap();
    assert_eq!(
        *dates.lock().unwrap(),
        vec!["20260101T000000Z", "20260101T000000Z"]
    );
}

#[tokio::test]
async fn sdk_presigning_uses_the_injected_wall_clock_and_ttl() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let cache = S3ClientCache::new(Arc::new(FixedClock(now)));
    let spec = S3StorageSpec {
        endpoint: "http://internal.invalid".into(),
        public_endpoint: "https://public.invalid".into(),
        region: "local".into(),
        bucket: "objects".into(),
        force_path_style: true,
        access_key: "access".into(),
        secret_key: secrecy::SecretString::from("secret".to_owned()),
    };
    let storage = cache.get("storage", &spec, Address::Public);
    for ttl in [1, 900, 604_800] {
        for url in [
            s3_presign_get(&storage, "key", None, Duration::from_secs(ttl))
                .await
                .unwrap(),
            s3_presign_put(&storage, "key", None, Duration::from_secs(ttl))
                .await
                .unwrap(),
            s3_presign_upload_part(&storage, "key", "upload", 1, Duration::from_secs(ttl))
                .await
                .unwrap(),
        ] {
            assert!(url.contains("X-Amz-Date=20260101T000000Z"), "{url}");
            assert!(url.contains(&format!("X-Amz-Expires={ttl}")), "{url}");
        }
    }
    assert!(
        s3_presign_get(&storage, "key", None, Duration::from_secs(604_801))
            .await
            .is_err()
    );
}
