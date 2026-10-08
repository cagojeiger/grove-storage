//! Durable retry scheduling for upload completion and cleanup.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum Job {
    NativeComplete,
    NativeCleanup,
    S3Complete,
    S3Cleanup,
}

impl Job {
    fn protocol(self) -> &'static str {
        match self {
            Self::NativeComplete | Self::NativeCleanup => "native",
            Self::S3Complete | Self::S3Cleanup => "s3",
        }
    }

    fn state(self) -> &'static str {
        match self {
            Self::NativeComplete | Self::S3Complete => "completing",
            Self::NativeCleanup | Self::S3Cleanup => "cleaning",
        }
    }
}

/// Persist the retry delay before provider I/O, including attempts interrupted by a crash.
pub async fn claim(
    pool: &PgPool,
    file_id: Uuid,
    job: Job,
    retry_secs: u32,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let file: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM files WHERE id=$1 AND state='pending' FOR UPDATE")
            .bind(file_id)
            .fetch_optional(&mut *tx)
            .await?;
    if file.is_none() {
        return Ok(false);
    }
    // Recheck ownership and lease after the file lock, using current rather than start time.
    let claimed = sqlx::query(
        "UPDATE uploads u SET recovery_after=grove_time.wall_now()+$4*interval '1 second' \
         WHERE u.file_id=$1 AND u.protocol=$2 AND u.state=$3 \
         AND u.recovery_after<=grove_time.wall_now() \
         AND ($3='cleaning' OR EXISTS (SELECT 1 FROM leases le \
              WHERE le.file_id=u.file_id AND le.kind='write' AND le.state='issued' \
              AND le.expires_at<grove_time.wall_now()))",
    )
    .bind(file_id)
    .bind(job.protocol())
    .bind(job.state())
    .bind(i64::from(retry_secs))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(claimed.rows_affected() == 1)
}
