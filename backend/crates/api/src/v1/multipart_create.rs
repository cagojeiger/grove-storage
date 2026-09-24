use filegate_db::files::{self, CreatedFile};
use filegate_infra::Address;
use grove_object_service::multipart_create::MultipartCreate;

use crate::error::{ApiError, internal};
use crate::routes::AppState;
use crate::storage_access::StorageBackend;

pub(super) struct Operations<'a> {
    pub state: &'a AppState,
    pub created: &'a CreatedFile,
    pub backend: &'a StorageBackend,
    pub content_type: Option<&'a str>,
}

impl MultipartCreate for Operations<'_> {
    type Error = ApiError;

    async fn create_vendor_upload(&self) -> Result<Option<String>, ApiError> {
        let StorageBackend::S3 { spec, .. } = self.backend else {
            return Ok(None);
        };
        let storage = self
            .state
            .s3_clients
            .get(&self.created.storage.id, spec, Address::Internal);
        filegate_infra::s3_create_multipart(&storage, &self.created.object_key, self.content_type)
            .await
            .map(Some)
            .map_err(ApiError::Storage)
    }

    async fn attach_vendor_upload(&self, upload_id: &str) -> Result<(), ApiError> {
        files::attach_upload_id(&self.state.pool, self.created.lease_id, upload_id).await?;
        Ok(())
    }

    async fn prepare_relay(&self) -> Result<(), ApiError> {
        if self.backend.is_relay() {
            let secret = self
                .state
                .crypto
                .relay_secret(&self.created.lease_id.to_string())
                .map_err(internal)?;
            files::attach_write_secret(
                &self.state.pool,
                self.created.lease_id,
                &filegate_core::client_key_hash(&secret),
            )
            .await?;
        }
        Ok(())
    }

    async fn compensate(&self, upload_id: Option<&str>) {
        cleanup_failed_multipart_create(self.state, self.created, self.backend, upload_id).await;
    }
}

/// create 응답 전에 실패한 multipart 예약은 클라이언트가 재개할 수 없다.
/// 외부 세션 정리가 확인된 뒤에만 DB pending을 닫는다. 벤더 Abort가 실패하면
/// location/lease를 남겨 만료 회수가 저장된 upload_id로 재시도할 수 있게 한다.
async fn cleanup_failed_multipart_create(
    state: &AppState,
    created: &CreatedFile,
    backend: &StorageBackend,
    vendor_upload_id: Option<&str>,
) {
    use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};

    let result = cleanup_then_finalize(
        || async {
            if let StorageBackend::S3 { spec, .. } = backend {
                let storage = state
                    .s3_clients
                    .get(&created.storage.id, spec, Address::Internal);
                match vendor_upload_id {
                    Some(upload_id) => {
                        filegate_infra::s3_abort_multipart(&storage, &created.object_key, upload_id)
                            .await?
                    }
                    None => {
                        filegate_infra::s3_abort_multipart_by_key(&storage, &created.object_key)
                            .await?
                    }
                }
            }
            Ok::<_, anyhow::Error>(())
        },
        || files::reclaim_pending(&state.pool, created.file_id),
    )
    .await;
    match result {
        Ok(_) => {}
        Err(CleanupError::Physical(error)) => tracing::warn!(
            event = "file.multipart_create_cleanup_failed",
            file = %created.file_id,
            step = "abort_vendor",
            error = %error,
        ),
        Err(CleanupError::Metadata(error)) => tracing::warn!(
            event = "file.multipart_create_cleanup_failed",
            file = %created.file_id,
            step = "reclaim_pending",
            error = %error,
        ),
    }
}
