//! S3 single/multipart completion recovery.

use grove_core::Crypto;
use grove_db::{PgPool, registry, s3_registry as s3reg, upload_recovery::Job};
use grove_infra::S3ClientCache;
use grove_object_policy::completion::{CompletionAction, completion_action};

use super::{BATCH_LIMIT, begin_recovery, recovery_io};
use crate::lease::WRITE_LEASE_TTL;
use crate::logging::worker::Report as JobReport;

pub(super) async fn recover(
    pool: &PgPool,
    crypto: &Crypto,
    s3_clients: &S3ClientCache,
) -> JobReport {
    let mut report = JobReport::default();
    let candidates = match s3reg::completion_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", error_code = "db_scan_failed", outcome = "error", job = "s3_complete", %error);
            report.failed += 1;
            return report;
        }
    };
    for candidate in candidates {
        if !begin_recovery(pool, candidate.file_id, Job::S3Complete, &mut report).await {
            continue;
        }
        let observation =
            recovery_io(observe_s3_completion(pool, crypto, s3_clients, &candidate)).await;
        let action = match completion_action(
            candidate.expected_size,
            &candidate.expected_etag,
            candidate.multipart,
            observation,
        ) {
            Ok(action) => action,
            Err(error) => {
                report.failed += 1;
                tracing::warn!(
                    event = "reconciler.observe_failed", error_code = "observation_failed", outcome = "error",
                    file = %candidate.file_id,
                    %error,
                );
                continue;
            }
        };
        match action {
            CompletionAction::Finalize => {
                let finalized = if candidate.multipart {
                    s3reg::finalize_multipart_upload(
                        pool,
                        &candidate.client_id,
                        &candidate.key,
                        candidate.file_id,
                    )
                    .await
                } else {
                    s3reg::finalize_single_upload(
                        pool,
                        &candidate.client_id,
                        &candidate.key,
                        candidate.file_id,
                    )
                    .await
                };
                match finalized {
                    Ok(s3reg::FinalizeOutcome::Finalized { .. }) => {
                        report.processed += 1;
                        tracing::info!(
                            event = "s3.upload_recovered",
                            file = %candidate.file_id,
                        )
                    }
                    Ok(s3reg::FinalizeOutcome::NotPending) => {}
                    Ok(s3reg::FinalizeOutcome::PreconditionFailed) => {
                        report.processed += 1;
                        tracing::info!(
                            event = "s3.precondition_failed",
                            file = %candidate.file_id,
                        )
                    }
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
            CompletionAction::Reopen => {
                match s3reg::reopen_completion(
                    pool,
                    candidate.file_id,
                    WRITE_LEASE_TTL.as_secs() as i64,
                )
                .await
                {
                    Ok(true) => {
                        report.processed += 1;
                        tracing::info!(
                            event = "s3.completion_reopened",
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
            CompletionAction::Cleanup => {
                match s3reg::mark_completion_aborting(pool, candidate.file_id).await {
                    Ok(true) => {
                        report.processed += 1;
                        tracing::warn!(
                            event = "s3.completion_invalid",
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

async fn observe_s3_completion(
    pool: &PgPool,
    crypto: &Crypto,
    s3_clients: &S3ClientCache,
    candidate: &s3reg::CompletionCandidate,
) -> anyhow::Result<Option<grove_object_policy::completion::ObjectObservation>> {
    let row = registry::get_storage(pool, &candidate.storage_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("storage '{}' not registered", candidate.storage_id))?;
    let backend = crate::storage_access::backend_from_row(crypto, &row)?;
    grove_infra::s3_io::observe_backend_object(
        s3_clients,
        &backend,
        &candidate.storage_id,
        &candidate.object_key,
    )
    .await
}
