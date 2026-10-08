//! Retry eligibility for files without an upload owner.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum RecoveryJob {
    Observe,
    Reclaim,
    Purge,
}

impl RecoveryJob {
    fn state(self) -> &'static str {
        match self {
            Self::Observe => "pending",
            Self::Reclaim => "reclaimed",
            Self::Purge => "deleted",
        }
    }
}

/// Commit the retry delay before I/O; cancellation leaves the intent and location intact.
pub async fn claim_recovery(
    pool: &PgPool,
    file_id: Uuid,
    job: RecoveryJob,
    retry_secs: u32,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let file: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM files WHERE id=$1 AND state=$2 FOR UPDATE")
            .bind(file_id)
            .bind(job.state())
            .fetch_optional(&mut *tx)
            .await?;
    if file.is_none() {
        return Ok(false);
    }
    // Ownership and lease may have changed while acquiring the file lock.
    let claimed = sqlx::query(
        "UPDATE files f SET recovery_after=grove_time.wall_now()+$3*interval '1 second' \
         WHERE f.id=$1 AND f.state=$2 AND f.recovery_after<=grove_time.wall_now() \
         AND NOT EXISTS (SELECT 1 FROM uploads u WHERE u.file_id=f.id) \
         AND EXISTS (SELECT 1 FROM locations l WHERE l.file_id=f.id) \
         AND ($2<>'pending' OR (f.part_size IS NULL AND EXISTS ( \
             SELECT 1 FROM leases le WHERE le.file_id=f.id AND le.kind='write' \
             AND le.state='issued' AND le.expires_at>grove_time.wall_now())))",
    )
    .bind(file_id)
    .bind(job.state())
    .bind(i64::from(retry_secs))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(claimed.rows_affected() == 1)
}
