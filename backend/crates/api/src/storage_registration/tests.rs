#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;

fn fs_body(force_path_style: bool) -> Submission {
    Submission {
        kind: "fs".to_owned(),
        force_relay: false,
        root_path: Some("/unused".to_owned()),
        endpoint: None,
        public_endpoint: None,
        region: None,
        bucket: None,
        force_path_style,
        access_key: None,
        secret_key: None,
        capacity_bytes: 1,
    }
}

#[test]
fn require_http_url_accepts_http_https_only() {
    assert!(require_http_url("http://minio:9000", "endpoint").is_ok());
    assert!(require_http_url("https://cdn.example.com", "public_endpoint").is_ok());
    assert!(require_http_url("ftp://x", "endpoint").is_err()); // http(s) 아닌 스킴
    assert!(require_http_url("minio:9000", "endpoint").is_err()); // 스킴 없음
    // 빈 host(authority)는 presign이 붙을 곳이 없다 — 거부한다.
    assert!(require_http_url("http:///path", "endpoint").is_err());
}

#[tokio::test]
async fn filesystem_submission_is_rejected_before_probe() {
    let state = crate::routes::tests::test_state();
    assert!(
        verified_row(
            &state.crypto,
            state.clock.clone(),
            true,
            "fs-test",
            fs_body(false)
        )
        .await
        .is_err()
    );
}

#[test]
fn legacy_submission_preserves_defaults_and_public_endpoint_fallback() {
    for public_endpoint in [None, Some("")] {
        let body: Submission = serde_json::from_value(serde_json::json!({
            "endpoint": "https://storage.example.com",
            "public_endpoint": public_endpoint,
            "region": "local", "bucket": "objects",
            "access_key": "key", "secret_key": "secret", "capacity_bytes": 0,
            "legacy_unknown_field": true,
        }))
        .unwrap();
        assert_eq!(body.kind, "s3");
        assert!(!body.force_relay);
        assert!(!body.force_path_style);
        let submission = validated_s3_submission(false, body)
            .unwrap_or_else(|_| panic!("valid legacy submission was rejected"));
        assert_eq!(submission.spec.public_endpoint, submission.spec.endpoint);
        assert_eq!(submission.capacity_bytes, 0);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn status_collects_all_checks_while_startup_rejects_failure(pool: PgPool) {
    use axum::{Router, http::Method, response::IntoResponse};
    let state = crate::routes::tests::test_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let provider = Router::new().fallback(|request: axum::extract::Request| async move {
        if request.uri().path().starts_with("/missing") {
            return StatusCode::FORBIDDEN.into_response();
        }
        if request.method() == Method::HEAD {
            return StatusCode::OK.into_response();
        }
        (StatusCode::OK, "<ListMultipartUploadsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><IsTruncated>false</IsTruncated></ListMultipartUploadsResult>").into_response()
    });
    let task = tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    for id in ["healthy", "missing"] {
        let row = encrypted_s3_row(
            &state.crypto,
            id,
            S3Submission {
                spec: S3StorageSpec {
                    endpoint: endpoint.clone(),
                    public_endpoint: endpoint.clone(),
                    region: "local".into(),
                    bucket: id.into(),
                    force_path_style: true,
                    access_key: "key".into(),
                    secret_key: "secret".to_owned().into(),
                },
                force_relay: false,
                capacity_bytes: 1,
            },
        )
        .unwrap_or_else(|_| panic!("fixture encryption failed"));
        registry::insert_storage(&pool, &row).await.unwrap();
    }

    let checks = check_registered(&pool, &state.crypto).await.unwrap();
    assert_eq!(checks.len(), 2);
    assert!(
        checks
            .iter()
            .find(|check| check.id == "healthy")
            .unwrap()
            .ok()
    );
    assert!(
        !checks
            .iter()
            .find(|check| check.id == "missing")
            .unwrap()
            .ok()
    );
    assert!(verify_registered(&pool, &state.crypto).await.is_err());
    registry::delete_storage(&pool, "missing").await.unwrap();
    assert!(verify_registered(&pool, &state.crypto).await.is_ok());
    task.abort();
}
