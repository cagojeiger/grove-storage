//! Physical S3 operations without database or HTTP state.
use std::path::Path;

use crate::{
    Address, S3ClientCache, S3StorageSpec, s3_abort_multipart, s3_abort_multipart_by_key,
    s3_delete_object, s3_head_object,
};
use grove_object_policy::completion::ObjectObservation;

pub struct StorageBackend {
    pub spec: S3StorageSpec,
    pub force_relay: bool,
}

impl StorageBackend {
    pub fn is_relay(&self) -> bool {
        self.force_relay
    }
}

pub async fn observe_backend_object(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    object_key: &str,
) -> anyhow::Result<Option<ObjectObservation>> {
    let storage = s3_clients.get(storage_id, &backend.spec, Address::Internal);
    Ok(s3_head_object(&storage, object_key)
        .await?
        .map(|(size, etag)| ObjectObservation {
            size,
            etag: Some(etag),
        }))
}

/// Attempt both multipart and final-object cleanup, even if one fails.
/// Recovery metadata is released by the caller only after both succeed.
pub async fn cleanup_backend_upload(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    object_key: &str,
    upload_id: Option<&str>,
    multipart: bool,
) -> anyhow::Result<()> {
    let storage = s3_clients.get(storage_id, &backend.spec, Address::Internal);
    let aborted = match (upload_id, multipart) {
        (Some(upload_id), _) => s3_abort_multipart(&storage, object_key, upload_id).await,
        (None, true) => s3_abort_multipart_by_key(&storage, object_key).await,
        (None, false) => Ok(()),
    };
    let deleted = s3_delete_object(&storage, object_key).await;
    aborted?;
    deleted
}

/// Upload spooled bytes without publishing metadata. Both successful and failed
/// provider attempts release the temporary file.
pub async fn commit_temp_to_backend(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    file: tokio::fs::File,
    temp_path: &Path,
    object_key: &str,
    content_type: Option<&str>,
) -> anyhow::Result<()> {
    drop(file);
    let storage = s3_clients.get(storage_id, &backend.spec, Address::Internal);
    let uploaded =
        crate::s3_put_object_from_path(&storage, object_key, temp_path, content_type).await;
    crate::temp_spool::abort_write(temp_path).await;
    uploaded
}
