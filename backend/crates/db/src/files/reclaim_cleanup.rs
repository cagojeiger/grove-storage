//! Durable physical cleanup after generic pending reclamation.

use super::SweepCandidate;
use sqlx::PgPool;

pub async fn reclaim_cleanup_candidates(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<SweepCandidate>, sqlx::Error> {
    sqlx::query_as(
        "SELECT f.id AS file_id, l.storage_id, l.object_key, le.upload_id, \
         le.id AS write_lease_id, f.part_size IS NOT NULL AS multipart \
         FROM files f JOIN locations l ON l.file_id = f.id \
         LEFT JOIN leases le ON le.file_id = f.id AND le.kind = 'write' \
         WHERE f.state = 'reclaimed' ORDER BY f.created_at, f.id LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Called only after physical cleanup. A stale or repeated finalize is a no-op.
pub async fn finalize_reclaim_cleanup(
    pool: &PgPool,
    candidate: &SweepCandidate,
) -> Result<bool, sqlx::Error> {
    let deleted = sqlx::query(
        "DELETE FROM locations l USING files f \
         WHERE l.file_id = f.id AND f.id = $1 AND f.state = 'reclaimed' \
         AND l.storage_id = $2 AND l.object_key = $3",
    )
    .bind(candidate.file_id)
    .bind(&candidate.storage_id)
    .bind(&candidate.object_key)
    .execute(pool)
    .await?;
    Ok(deleted.rows_affected() > 0)
}
