use super::{BATCH_LIMIT, sweep_object};
use filegate_core::Crypto;
use filegate_db::{PgPool, files};
use filegate_infra::S3ClientCache;
use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};

#[cfg(test)]
mod tests;

pub(super) async fn recover(pool: &PgPool, crypto: &Crypto, s3_clients: &S3ClientCache) {
    let candidates = match files::reclaim_cleanup_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", job = "reclaim_cleanup", %error);
            return;
        }
    };
    for candidate in candidates {
        match cleanup_then_finalize(
            || sweep_object(pool, crypto, s3_clients, &candidate),
            || files::finalize_reclaim_cleanup(pool, &candidate),
        )
        .await
        {
            Ok(true) => tracing::info!(event = "file.reclaim_cleaned", file = %candidate.file_id),
            Ok(false) => {}
            Err(CleanupError::Physical(error)) => {
                tracing::warn!(event = "reconciler.sweep_failed", file = %candidate.file_id, %error)
            }
            Err(CleanupError::Metadata(error)) => {
                tracing::error!(event = "reconciler.reclaim_failed", file = %candidate.file_id, %error)
            }
        }
    }
}
