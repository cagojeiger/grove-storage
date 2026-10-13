//! Native multipart completion recovery.

use grove_core::Crypto;
use grove_db::{PgPool, files, registry, upload_recovery::Job};
use grove_infra::S3ClientCache;
use grove_object_policy::completion::{CompletionAction, completion_action};
use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};

use super::{BATCH_LIMIT, begin_recovery, recovery_io, sweep_object};
use crate::lease::WRITE_LEASE_TTL;
use crate::logging::worker::Report as JobReport;

pub(super) async fn recover(
    pool: &PgPool,
    crypto: &Crypto,
    s3_clients: &S3ClientCache,
) -> JobReport {
    let mut report = JobReport::default();
    let candidates = match files::completion_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", error_code = "db_scan_failed", outcome = "error", job = "native_complete", %error);
            report.failed += 1;
            return report;
        }
    };
    for candidate in candidates {
        if !begin_recovery(pool, candidate.file_id, Job::NativeComplete, &mut report).await {
            continue;
        }
        let row = match registry::get_storage(pool, &candidate.storage_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                report.failed += 1;
                tracing::error!(
                    event = "reconciler.observe_failed", error_code = "observation_failed", outcome = "error",
                    file = %candidate.file_id,
                    error = "storage is not registered",
                );
                continue;
            }
            Err(error) => {
                report.failed += 1;
                tracing::warn!(event = "reconciler.observe_failed", error_code = "observation_failed", outcome = "error", file = %candidate.file_id, %error);
                continue;
            }
        };
        let backend = match crate::storage_access::backend_from_row(crypto, &row) {
            Ok(backend) => backend,
            Err(error) => {
                report.failed += 1;
                tracing::warn!(event = "reconciler.observe_failed", error_code = "observation_failed", outcome = "error", file = %candidate.file_id, %error);
                continue;
            }
        };
        let observation = recovery_io(grove_infra::s3_io::observe_backend_object(
            s3_clients,
            &backend,
            &candidate.storage_id,
            &candidate.object_key,
        ))
        .await;
        let action = match completion_action(
            candidate.expected_size,
            &candidate.expected_etag,
            true,
            observation,
        ) {
            Ok(action) => action,
            Err(error) => {
                report.failed += 1;
                tracing::warn!(event = "reconciler.observe_failed", error_code = "observation_failed", outcome = "error", file = %candidate.file_id, %error);
                continue;
            }
        };
        match action {
            CompletionAction::Finalize => {
                match files::finalize_completion(pool, candidate.file_id, &candidate.expected_etag)
                    .await
                {
                    Ok(true) => {
                        report.processed += 1;
                        tracing::info!(
                            event = "file.multipart_recovered",
                            file = %candidate.file_id,
                        )
                    }
                    Ok(false) => {}
                    Err(error) => {
                        report.failed += 1;
                        tracing::error!(
                            event = "reconciler.commit_failed", error_code = "db_commit_failed", outcome = "error",
                            file = %candidate.file_id,
                            %error,
                        )
                    }
                }
            }
            CompletionAction::Reopen => match files::reopen_completion(
                pool,
                candidate.file_id,
                WRITE_LEASE_TTL.as_secs() as i64,
            )
            .await
            {
                Ok(true) => {
                    report.processed += 1;
                    tracing::info!(
                        event = "file.multipart_completion_reopened",
                        file = %candidate.file_id,
                    )
                }
                Ok(false) => {}
                Err(error) => {
                    report.failed += 1;
                    tracing::error!(
                        event = "reconciler.commit_failed", error_code = "db_commit_failed", outcome = "error",
                        file = %candidate.file_id,
                        %error,
                    )
                }
            },
            CompletionAction::Cleanup => {
                match files::claim_cleanup(pool, candidate.file_id).await {
                    Ok(true) => {
                        report.processed += 1;
                        tracing::warn!(
                            event = "file.multipart_completion_invalid",
                            file = %candidate.file_id,
                        )
                    }
                    Ok(false) => {}
                    Err(error) => {
                        report.failed += 1;
                        tracing::error!(
                            event = "reconciler.reclaim_failed", error_code = "db_reclaim_failed", outcome = "error",
                            file = %candidate.file_id,
                            %error,
                        )
                    }
                }
            }
        }
    }
    report
}

pub(super) async fn cleanup(
    pool: &PgPool,
    crypto: &Crypto,
    s3_clients: &S3ClientCache,
) -> JobReport {
    let mut report = JobReport::default();
    let cleanup = match files::completion_cleanup_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", error_code = "db_scan_failed", outcome = "error", job = "native_complete_cleanup", %error);
            report.failed += 1;
            return report;
        }
    };
    for candidate in cleanup {
        if !begin_recovery(pool, candidate.file_id, Job::NativeCleanup, &mut report).await {
            continue;
        }
        match cleanup_then_finalize(
            || recovery_io(sweep_object(pool, crypto, s3_clients, &candidate)),
            || files::finalize_completion_cleanup(pool, candidate.file_id),
        )
        .await
        {
            Ok(true) => {
                report.processed += 1;
                tracing::info!(
                    event = "file.multipart_completion_cleaned",
                    file = %candidate.file_id,
                )
            }
            Ok(false) => {}
            Err(CleanupError::Metadata(error)) => {
                report.failed += 1;
                tracing::error!(
                    event = "reconciler.reclaim_failed", error_code = "db_reclaim_failed", outcome = "error",
                    file = %candidate.file_id,
                    %error,
                )
            }
            Err(CleanupError::Physical(error)) => {
                report.failed += 1;
                tracing::warn!(
                    event = "reconciler.sweep_failed", error_code = "provider_cleanup_failed", outcome = "error",
                    file = %candidate.file_id,
                    %error,
                )
            }
        }
    }
    report
}
