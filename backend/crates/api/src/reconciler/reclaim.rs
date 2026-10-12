use super::{BATCH_LIMIT, begin_file_recovery, recovery_io, sweep_object};
use crate::metrics::JobReport;
use grove_core::Crypto;
use grove_db::{PgPool, files};
use grove_infra::S3ClientCache;
use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};

#[cfg(test)]
mod tests;

pub(super) async fn recover(
    pool: &PgPool,
    crypto: &Crypto,
    s3_clients: &S3ClientCache,
) -> JobReport {
    let mut report = JobReport::default();
    let candidates = match files::reclaim_cleanup_candidates(pool, BATCH_LIMIT).await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::error!(event = "reconciler.scan_failed", job = "reclaim_cleanup", %error);
            report.failed += 1;
            return report;
        }
    };
    for candidate in candidates {
        if !begin_file_recovery(
            pool,
            candidate.file_id,
            files::RecoveryJob::Reclaim,
            &mut report,
        )
        .await
        {
            continue;
        }
        match cleanup_then_finalize(
            || recovery_io(sweep_object(pool, crypto, s3_clients, &candidate)),
            || files::finalize_reclaim_cleanup(pool, &candidate),
        )
        .await
        {
            Ok(true) => {
                report.processed += 1;
                tracing::info!(event = "file.reclaim_cleaned", file = %candidate.file_id)
            }
            Ok(false) => {}
            Err(CleanupError::Physical(error)) => {
                report.failed += 1;
                tracing::warn!(event = "reconciler.sweep_failed", file = %candidate.file_id, %error)
            }
            Err(CleanupError::Metadata(error)) => {
                report.failed += 1;
                tracing::error!(event = "reconciler.reclaim_failed", file = %candidate.file_id, %error)
            }
        }
    }
    report
}
