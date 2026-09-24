//! Single relay PUT ownership: file lock spans receive, storage IO, and measurements.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct RelayUploadClaim {
    tx: Transaction<'static, Postgres>,
    lease_id: Uuid,
    pub recorded: Option<(i64, String)>,
    pub declared_md5: Option<String>,
}

/// Admission is bounded by the caller. A competing upload gets no claim, while
/// commit/reclaim wait on the same file row. Recheck expiration after acquiring it.
pub async fn claim_relay_upload(
    pool: &PgPool,
    file_id: Uuid,
    lease_id: Uuid,
) -> Result<Option<RelayUploadClaim>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let file: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT declared_md5 FROM files WHERE id = $1 AND state = 'pending' \
         AND part_size IS NULL FOR UPDATE SKIP LOCKED",
    )
    .bind(file_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((declared_md5,)) = file else {
        return Ok(None);
    };
    let lease: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT uploaded_size, uploaded_md5 FROM leases \
         WHERE id = $1 AND file_id = $2 AND kind = 'write' AND state = 'issued' \
         AND expires_at > clock_timestamp() AND secret_hash IS NOT NULL \
         AND NOT EXISTS (SELECT 1 FROM s3_uploads WHERE file_id = $2) \
         AND NOT EXISTS (SELECT 1 FROM native_multipart_completions WHERE file_id = $2) \
         FOR UPDATE",
    )
    .bind(lease_id)
    .bind(file_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((size, md5)) = lease else {
        return Ok(None);
    };
    Ok(Some(RelayUploadClaim {
        tx,
        lease_id,
        recorded: size.zip(md5),
        declared_md5,
    }))
}

impl RelayUploadClaim {
    pub async fn done(mut self, size: i64, md5: &str, ttl_secs: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE leases SET uploaded_size = $2, uploaded_md5 = $3, \
             expires_at = GREATEST(expires_at, clock_timestamp() + $4 * interval '1 second') \
             WHERE id = $1",
        )
        .bind(self.lease_id)
        .bind(size)
        .bind(md5)
        .bind(ttl_secs)
        .execute(&mut *self.tx)
        .await?;
        self.tx.commit().await
    }
}
