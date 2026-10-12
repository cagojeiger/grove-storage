//! Read-only operational snapshots; no accounting counters or object identity labels.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
pub struct RecoveryQueue {
    pub job: String,
    pub objects: i64,
    pub unknown_age_objects: i64,
    pub oldest_at: Option<DateTime<Utc>>,
}

pub async fn snapshot(
    pool: &PgPool,
) -> Result<(Vec<crate::usage::StorageUsage>, Vec<RecoveryQueue>), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for statement in [
        "SET TRANSACTION READ ONLY",
        "SET LOCAL statement_timeout='1s'",
        "SET LOCAL lock_timeout='250ms'",
    ] {
        sqlx::query(statement).execute(&mut *tx).await?;
    }
    let usage = crate::usage::by_storage(&mut *tx).await?;
    let queues = recovery_queues(&mut *tx).await?;
    tx.commit().await?;
    Ok((usage, queues))
}

pub async fn recovery_queues<'e>(
    pool: impl sqlx::PgExecutor<'e>,
) -> Result<Vec<RecoveryQueue>, sqlx::Error> {
    sqlx::query_as(
        "WITH queued AS ( \
         SELECT 'cleanup' AS job, CASE WHEN f.state='deleted' THEN f.deleted_at ELSE \
         (SELECT max(le.expires_at) FROM leases le WHERE le.file_id=f.id AND le.kind='write' \
          AND le.expires_at<grove_time.transaction_now()) END AS entered_at FROM files f \
         WHERE f.state IN ('deleted','reclaimed') \
         AND EXISTS (SELECT 1 FROM locations l WHERE l.file_id=f.id) \
         AND NOT EXISTS (SELECT 1 FROM uploads u WHERE u.file_id=f.id) \
         UNION ALL SELECT 'cleanup', u.updated_at FROM uploads u WHERE u.state='cleaning' \
         UNION ALL SELECT CASE WHEN u.state='completing' THEN 'completion' ELSE 'expired_upload' END, \
         GREATEST(u.updated_at,le.expires_at) FROM uploads u \
         JOIN files f ON f.id=u.file_id \
         JOIN leases le ON le.file_id=u.file_id AND le.kind='write' AND le.state='issued' \
         WHERE f.state='pending' AND le.expires_at < grove_time.transaction_now() \
         AND (u.state='completing' OR (u.state='open' AND u.protocol='s3'))) \
         SELECT jobs.job, count(q.job)::bigint AS objects, \
         count(q.job) FILTER (WHERE q.entered_at IS NULL)::bigint AS unknown_age_objects, min(q.entered_at) AS oldest_at \
         FROM (VALUES ('cleanup'),('completion'),('expired_upload')) jobs(job) \
         LEFT JOIN queued q ON q.job=jobs.job GROUP BY jobs.job ORDER BY jobs.job"
    ).fetch_all(pool).await
}
