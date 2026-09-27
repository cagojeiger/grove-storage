//! Database/provider adapter for native single-upload completion.
use filegate_db::files::{self, FileAccess};
use filegate_infra::{Address, backend::StorageBackend, s3_head_object};
use grove_object_service::single_commit::{CommitError, SingleCommit};
use uuid::Uuid;

use super::ClientId;
use crate::{
    error::{ApiError, bad_request, conflict, not_found},
    routes::AppState,
};

pub(super) struct Operations<'a> {
    pub state: &'a AppState,
    pub client: &'a ClientId,
    pub file_id: Uuid,
    pub file: &'a FileAccess,
    pub backend: &'a StorageBackend,
}

impl SingleCommit for Operations<'_> {
    type Error = ApiError;

    async fn observe(&self) -> Result<Option<(i64, String)>, ApiError> {
        if self.backend.is_relay() {
            return Ok(files::recorded_upload(&self.state.pool, self.file_id).await?);
        }
        let spec = &self.backend.spec;
        let storage = self
            .state
            .s3_clients
            .get(&self.file.storage.id, spec, Address::Internal);
        s3_head_object(&storage, &self.file.object_key)
            .await
            .map_err(ApiError::Storage)
    }

    async fn finalize(&self, etag: &str) -> Result<bool, ApiError> {
        Ok(files::finalize_commit(&self.state.pool, self.file_id, etag).await?)
    }

    async fn committed_etag(&self) -> Result<Option<String>, ApiError> {
        current_etag(self.state, self.client, self.file_id).await
    }
}

/// Shared with multipart's lost-transition response; missing ownership stays 404.
pub(super) async fn current_etag(
    state: &AppState,
    client: &ClientId,
    file_id: Uuid,
) -> Result<Option<String>, ApiError> {
    let now = files::access(&state.pool, &client.0, file_id)
        .await?
        .ok_or_else(|| not_found("file not found"))?;
    Ok((now.state == "active").then(|| now.etag.unwrap_or_default()))
}

pub(super) fn error(error: CommitError<ApiError>) -> ApiError {
    match error {
        CommitError::Operation(error) => error,
        CommitError::MissingUpload => bad_request("no uploaded object to commit"),
        CommitError::SizeMismatch => bad_request("uploaded size does not match declaration"),
        CommitError::Md5Mismatch => bad_request("uploaded content does not match declared md5"),
        CommitError::NotCommittable => conflict("file is not committable"),
    }
}
