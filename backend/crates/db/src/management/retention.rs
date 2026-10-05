//! Server-owned retention; never an account-authorized deletion endpoint.
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use std::num::NonZeroU16;

const RETENTION_LOCK_KEY: i64 = 0x4753_4d47_4d54_4c47;

/// Independent of object reconciliation; transaction drop releases the lock.
pub async fn with_lock<F, Fut, T>(pool: &PgPool, job: F) -> Result<Option<T>, sqlx::Error>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    crate::with_advisory_lock(pool, RETENTION_LOCK_KEY, job).await
}

#[derive(Clone, Copy)]
pub enum Stream {
    Audit,
    Security,
    Invocations,
}
impl Stream {
    pub fn name(self) -> &'static str {
        match self {
            Self::Audit => "audit_events",
            Self::Security => "security_events",
            Self::Invocations => "command_invocations",
        }
    }
}

pub async fn prune(
    pool: &PgPool,
    stream: Stream,
    days: NonZeroU16,
    limit: NonZeroU16,
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '250ms'")
        .execute(&mut *tx)
        .await?;
    let cutoff =
        sqlx::query_scalar("SELECT grove_time.transaction_now() - $1 * interval '24 hours'")
            .bind(i32::from(days.get()))
            .fetch_one(&mut *tx)
            .await?;
    let deleted = delete_before(&mut tx, stream, cutoff, limit).await?;
    tx.commit().await?;
    Ok(deleted)
}

async fn delete_before(
    tx: &mut Transaction<'_, Postgres>,
    stream: Stream,
    cutoff: DateTime<Utc>,
    limit: NonZeroU16,
) -> Result<u64, sqlx::Error> {
    let sql = match stream {
        Stream::Audit => {
            "DELETE FROM management.audit_events WHERE id IN (
            SELECT id FROM management.audit_events WHERE created_at < $1
            ORDER BY created_at,id LIMIT $2)"
        }
        Stream::Security => {
            "DELETE FROM management.security_events WHERE id IN (
            SELECT id FROM management.security_events WHERE created_at < $1
            ORDER BY created_at,id LIMIT $2)"
        }
        Stream::Invocations => {
            "DELETE FROM management.command_invocations WHERE id IN (
            SELECT id FROM management.command_invocations WHERE created_at < $1
            ORDER BY created_at,id LIMIT $2)"
        }
    };
    Ok(sqlx::query(sql)
        .bind(cutoff)
        .bind(i64::from(limit.get()))
        .execute(&mut **tx)
        .await?
        .rows_affected())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn strict_cutoff_oldest_first_and_batch_limit(pool: PgPool) {
        let cutoff = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        sqlx::raw_sql("INSERT INTO management.security_events(created_at,actor_kind,request_id,surface,event_type,reason_code)
            VALUES ('2026-01-01T00:00:00Z','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated'),
                   ('2025-12-31T23:59:59.999999Z','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated'),
                   ('2025-01-01T00:00:00Z','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated'),
                   ('2026-01-01T00:00:00.000001Z','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated')")
            .execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            delete_before(
                &mut tx,
                Stream::Security,
                cutoff,
                NonZeroU16::new(1).unwrap()
            )
            .await
            .unwrap(),
            1
        );
        assert!(!sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM management.security_events WHERE created_at='2025-01-01T00:00:00Z')").fetch_one(&mut *tx).await.unwrap());
        assert_eq!(
            delete_before(
                &mut tx,
                Stream::Security,
                cutoff,
                NonZeroU16::new(10).unwrap()
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            delete_before(
                &mut tx,
                Stream::Security,
                cutoff,
                NonZeroU16::new(10).unwrap()
            )
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM management.security_events")
                .fetch_one(&mut *tx)
                .await
                .unwrap(),
            2
        );
        tx.commit().await.unwrap();
    }
}
