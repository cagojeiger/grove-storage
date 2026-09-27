//! Physical operations on a resolved backend, without database or HTTP state.
use std::path::{Path, PathBuf};

use crate::{
    Address, S3ClientCache, S3StorageSpec, s3_abort_multipart, s3_abort_multipart_by_key,
    s3_delete_object, s3_head_object,
};
use grove_object_policy::completion::ObjectObservation;

pub enum StorageBackend {
    S3 {
        spec: S3StorageSpec,
        force_relay: bool,
    },
    Fs {
        root: PathBuf,
    },
}

impl StorageBackend {
    pub fn is_relay(&self) -> bool {
        match self {
            Self::S3 { force_relay, .. } => *force_relay,
            Self::Fs { .. } => true,
        }
    }
}

/// The HTTP adapter maps local and provider failures to its own protocol.
pub enum CommitErr {
    Fs(anyhow::Error),
    Storage(anyhow::Error),
}

pub async fn observe_backend_object(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    object_key: &str,
) -> anyhow::Result<Option<ObjectObservation>> {
    match backend {
        StorageBackend::S3 { spec, .. } => {
            let storage = s3_clients.get(storage_id, spec, Address::Internal);
            Ok(s3_head_object(&storage, object_key)
                .await?
                .map(|(size, etag)| ObjectObservation {
                    size,
                    etag: Some(etag),
                }))
        }
        StorageBackend::Fs { root } => Ok(crate::fs::head_object(root, object_key)
            .await?
            .map(|size| ObjectObservation { size, etag: None })),
    }
}

/// Attempt both temporary-upload and final-object cleanup, even if one fails.
/// The caller releases recovery metadata only after both operations succeed.
pub async fn cleanup_backend_upload(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    object_key: &str,
    upload_id: Option<&str>,
    write_lease_id: Option<uuid::Uuid>,
    multipart: bool,
) -> anyhow::Result<()> {
    match backend {
        StorageBackend::S3 { spec, .. } => {
            let storage = s3_clients.get(storage_id, spec, Address::Internal);
            let aborted = match (upload_id, multipart) {
                (Some(upload_id), _) => s3_abort_multipart(&storage, object_key, upload_id).await,
                (None, true) => s3_abort_multipart_by_key(&storage, object_key).await,
                (None, false) => Ok(()),
            };
            let deleted = s3_delete_object(&storage, object_key).await;
            aborted?;
            deleted
        }
        StorageBackend::Fs { root } => {
            let temps = match (write_lease_id, multipart) {
                (Some(lease_id), true) => {
                    crate::fs::delete_multipart_temps(root, &lease_id.to_string()).await
                }
                _ => Ok(()),
            };
            let deleted = crate::fs::delete(root, object_key).await;
            temps?;
            deleted
        }
    }
}

/// Commit spooled bytes without publishing object metadata. Failed filesystem
/// commits and all completed S3 upload attempts release their temporary file.
pub async fn commit_temp_to_backend(
    s3_clients: &S3ClientCache,
    backend: &StorageBackend,
    storage_id: &str,
    file: tokio::fs::File,
    temp_path: &Path,
    object_key: &str,
    content_type: Option<&str>,
) -> Result<(), CommitErr> {
    match backend {
        StorageBackend::Fs { root } => {
            if let Err(error) = crate::fs::commit_write(file, temp_path, root, object_key).await {
                crate::fs::abort_write(temp_path).await;
                return Err(CommitErr::Fs(error));
            }
            Ok(())
        }
        StorageBackend::S3 { spec, .. } => {
            drop(file);
            let storage = s3_clients.get(storage_id, spec, Address::Internal);
            let uploaded =
                crate::s3_put_object_from_path(&storage, object_key, temp_path, content_type).await;
            crate::fs::abort_write(temp_path).await;
            uploaded.map_err(CommitErr::Storage)
        }
    }
}
