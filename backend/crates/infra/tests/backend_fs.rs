#![allow(clippy::unwrap_used)]
use filegate_infra::{
    S3ClientCache,
    backend::{
        CommitErr, StorageBackend, cleanup_backend_upload, commit_temp_to_backend,
        observe_backend_object,
    },
};

#[tokio::test]
async fn historical_filesystem_commit_observe_and_cleanup_remain_idempotent() {
    let root = std::env::temp_dir().join(format!("grove-backend-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&root).await.unwrap();
    let backend = StorageBackend::Fs { root: root.clone() };
    let cache = S3ClientCache::default();
    assert!(backend.is_relay());
    assert!(
        observe_backend_object(&cache, &backend, "home", "file")
            .await
            .unwrap()
            .is_none()
    );
    let (temp, file) = filegate_infra::fs::begin_write(&root, "test")
        .await
        .unwrap();
    tokio::fs::write(&temp, b"payload").await.unwrap();
    assert!(
        commit_temp_to_backend(&cache, &backend, "home", file, &temp, "file", None)
            .await
            .is_ok()
    );
    assert!(!temp.exists());
    let observed = observe_backend_object(&cache, &backend, "home", "file")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.size, 7);
    assert_eq!(observed.etag, None);
    for _ in 0..2 {
        cleanup_backend_upload(&cache, &backend, "home", "file", None, None, false)
            .await
            .unwrap();
    }
    assert!(!root.join("file").exists());
    tokio::fs::remove_dir_all(&root).await.unwrap();
}

#[tokio::test]
async fn failed_filesystem_commit_removes_temporary_bytes() {
    let root = std::env::temp_dir().join(format!("grove-backend-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&root).await.unwrap();
    let backend = StorageBackend::Fs { root: root.clone() };
    let (temp, file) = filegate_infra::fs::begin_write(&root, "test")
        .await
        .unwrap();
    let result = commit_temp_to_backend(
        &S3ClientCache::default(),
        &backend,
        "home",
        file,
        &temp,
        "../outside",
        None,
    )
    .await;
    assert!(matches!(result, Err(CommitErr::Fs(_))));
    assert!(!temp.exists());
    tokio::fs::remove_dir_all(&root).await.unwrap();
}
