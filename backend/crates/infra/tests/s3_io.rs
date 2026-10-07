#![allow(clippy::unwrap_used)]
mod support;

use grove_infra::{
    S3ClientCache,
    s3_io::{cleanup_backend_upload, commit_temp_to_backend, observe_backend_object},
};

#[tokio::test]
async fn observation_uses_internal_endpoint_and_preserves_size_and_etag() {
    let provider = support::Provider::start(false).await;
    let observed =
        observe_backend_object(&S3ClientCache::default(), &provider.backend, "home", "file")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(observed.size, 7);
    assert_eq!(observed.etag, "abc123");
    assert_eq!(*provider.requests.lock().unwrap(), ["HEAD /objects/file"]);
}

#[tokio::test]
async fn abort_failure_still_attempts_object_deletion_and_reports_failure() {
    let provider = support::Provider::start(true).await;
    let result = cleanup_backend_upload(
        &S3ClientCache::default(),
        &provider.backend,
        "home",
        "file",
        Some("upload"),
        true,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        *provider.requests.lock().unwrap(),
        ["DELETE /objects/file (multipart)", "DELETE /objects/file",]
    );
}

#[tokio::test]
async fn single_put_cleanup_only_deletes_the_object() {
    let provider = support::Provider::start(false).await;
    cleanup_backend_upload(
        &S3ClientCache::default(),
        &provider.backend,
        "home",
        "file",
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(*provider.requests.lock().unwrap(), ["DELETE /objects/file"]);
}

#[tokio::test]
async fn completed_s3_upload_attempts_always_remove_the_spool() {
    for fail in [false, true] {
        let provider = support::Provider::start(fail).await;
        let path = std::env::temp_dir().join(format!("grove-backend-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"payload").await.unwrap();
        let file = tokio::fs::File::open(&path).await.unwrap();
        let result = commit_temp_to_backend(
            &S3ClientCache::default(),
            &provider.backend,
            "home",
            file,
            &path,
            "file",
            Some("text/plain"),
        )
        .await;
        if fail {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok());
        }
        assert!(!path.exists());
        assert_eq!(*provider.requests.lock().unwrap(), ["PUT /objects/file"]);
    }
}

#[tokio::test]
async fn s3_relay_mode_follows_the_registered_setting() {
    let mut provider = support::Provider::start(false).await;
    assert!(!provider.backend.is_relay());
    provider.backend.force_relay = true;
    assert!(provider.backend.is_relay());
    assert!(provider.requests.lock().unwrap().is_empty());
}
