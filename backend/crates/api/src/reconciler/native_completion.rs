//! Native multipart completion recovery.

use filegate_core::Crypto;
use filegate_db::{PgPool, files, registry};
use filegate_infra::S3ClientCache;
use grove_object_policy::completion::{CompletionAction, completion_action};
use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};

use super::{BATCH_LIMIT, sweep_object};
use crate::lease::WRITE_LEASE_TTL;

pub(super) async fn recover(pool: &PgPool, crypto: &Crypto, s3_clients: &S3ClientCache) {
    let candidates = match files::completion_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", job = "native_complete", %error);
            return;
        }
    };
    for candidate in candidates {
        let row = match registry::get_storage(pool, &candidate.storage_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                tracing::error!(
                    event = "reconciler.observe_failed",
                    file = %candidate.file_id,
                    error = "storage is not registered",
                );
                continue;
            }
            Err(error) => {
                tracing::warn!(event = "reconciler.observe_failed", file = %candidate.file_id, %error);
                continue;
            }
        };
        let backend = match crate::storage_access::backend_from_row(crypto, &row) {
            Ok(backend) => backend,
            Err(error) => {
                tracing::warn!(event = "reconciler.observe_failed", file = %candidate.file_id, %error);
                continue;
            }
        };
        let observation = crate::storage_access::observe_backend_object(
            s3_clients,
            &backend,
            &candidate.storage_id,
            &candidate.object_key,
        )
        .await;
        let action = match completion_action(
            candidate.expected_size,
            &candidate.expected_etag,
            true,
            observation,
        ) {
            Ok(action) => action,
            Err(error) => {
                tracing::warn!(event = "reconciler.observe_failed", file = %candidate.file_id, %error);
                continue;
            }
        };
        match action {
            CompletionAction::Finalize => {
                match files::finalize_completion(pool, candidate.file_id, &candidate.expected_etag)
                    .await
                {
                    Ok(true) => tracing::info!(
                        event = "file.multipart_recovered",
                        file = %candidate.file_id,
                    ),
                    Ok(false) => {}
                    Err(error) => tracing::error!(
                        event = "reconciler.commit_failed",
                        file = %candidate.file_id,
                        %error,
                    ),
                }
            }
            CompletionAction::Reopen => match files::reopen_completion(
                pool,
                candidate.file_id,
                WRITE_LEASE_TTL.as_secs() as i64,
            )
            .await
            {
                Ok(true) => tracing::info!(
                    event = "file.multipart_completion_reopened",
                    file = %candidate.file_id,
                ),
                Ok(false) => {}
                Err(error) => tracing::error!(
                    event = "reconciler.commit_failed",
                    file = %candidate.file_id,
                    %error,
                ),
            },
            CompletionAction::Cleanup => {
                match files::claim_cleanup(pool, candidate.file_id).await {
                    Ok(true) => tracing::warn!(
                        event = "file.multipart_completion_invalid",
                        file = %candidate.file_id,
                    ),
                    Ok(false) => {}
                    Err(error) => tracing::error!(
                        event = "reconciler.reclaim_failed",
                        file = %candidate.file_id,
                        %error,
                    ),
                }
            }
        }
    }

    let cleanup = match files::completion_cleanup_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", job = "native_complete_cleanup", %error);
            return;
        }
    };
    for candidate in cleanup {
        match cleanup_then_finalize(
            || sweep_object(pool, crypto, s3_clients, &candidate),
            || files::finalize_completion_cleanup(pool, candidate.file_id),
        )
        .await
        {
            Ok(true) => tracing::info!(
                event = "file.multipart_completion_cleaned",
                file = %candidate.file_id,
            ),
            Ok(false) => {}
            Err(CleanupError::Metadata(error)) => tracing::error!(
                event = "reconciler.reclaim_failed",
                file = %candidate.file_id,
                %error,
            ),
            Err(CleanupError::Physical(error)) => tracing::warn!(
                event = "reconciler.sweep_failed",
                file = %candidate.file_id,
                %error,
            ),
        }
    }
}
