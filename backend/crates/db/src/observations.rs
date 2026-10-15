//! Read-only operational snapshots, collected outside request and scrape paths.
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::num::NonZeroU16;

use crate::retention::{self, Stream};

pub fn pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy(database_url)
}

#[derive(Debug)]
pub struct Snapshot {
    pub observed_at: DateTime<Utc>,
    pub objects: Vec<ObjectBacklog>,
    pub retention: Vec<retention::Backlog>,
    pub tables: Vec<Table>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ObjectBacklog {
    pub queue: String,
    pub items: i64,
    pub oldest_waiting_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Table {
    pub schema: String,
    pub table: String,
    pub data_bytes: i64,
    pub index_bytes: i64,
    pub estimated_live_rows: i64,
    pub estimated_dead_rows: i64,
}

#[derive(Debug)]
pub struct Failure {
    pub stage: &'static str,
    pub stream: Option<&'static str>,
    pub reason: &'static str,
}

impl Failure {
    fn new(stage: &'static str, stream: Option<&'static str>, error: sqlx::Error) -> Self {
        let reason = match &error {
            sqlx::Error::PoolTimedOut => "pool_timeout",
            sqlx::Error::PoolClosed => "pool_closed",
            sqlx::Error::Io(_) | sqlx::Error::Tls(_) => "connection_failed",
            sqlx::Error::Database(error) => match error.code().as_deref() {
                Some("55P03") => "lock_timeout",
                Some("57014") => "query_canceled",
                Some("42501") => "permission_denied",
                Some("25006") => "read_only_violation",
                Some("42P01" | "42703" | "3F000") => "schema_mismatch",
                Some(code) if code.starts_with("08") => "connection_failed",
                _ => "database_error",
            },
            sqlx::Error::Decode(_)
            | sqlx::Error::ColumnDecode { .. }
            | sqlx::Error::ColumnNotFound(_)
            | sqlx::Error::TypeNotFound { .. } => "decode_failed",
            _ => "collection_failed",
        };
        // Discard the original error, which can contain SQL, values or connection details.
        Self {
            stage,
            stream,
            reason,
        }
    }
}

pub async fn collect(
    pool: &PgPool,
    retention: [(Stream, NonZeroU16); 8],
) -> Result<Snapshot, Failure> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| Failure::new("begin", None, error))?;
    for statement in [
        "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY",
        "SET LOCAL statement_timeout = '2s'",
        "SET LOCAL lock_timeout = '250ms'",
    ] {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(|error| Failure::new("configure", None, error))?;
    }
    let observed_at = sqlx::query_scalar("SELECT grove_time.transaction_now()")
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| Failure::new("clock", None, error))?;
    // Backoff does not remove outstanding work or reset its age.
    let objects = sqlx::query_as(
        "WITH candidates AS (
            SELECT CASE WHEN u.file_id IS NULL THEN 'expired_pending'
                        WHEN u.state='open' THEN 's3_expired'
                        ELSE u.protocol || '_complete' END AS queue, le.expires_at AS waiting_at
            FROM public.files f JOIN public.leases le ON le.file_id=f.id
            LEFT JOIN public.uploads u ON u.file_id=f.id
            WHERE f.state='pending' AND le.kind='write' AND le.state='issued'
                AND le.expires_at < grove_time.transaction_now()
                AND (u.file_id IS NULL OR u.state IN ('open','completing'))
            UNION ALL
            SELECT u.protocol || '_cleanup', u.updated_at
            FROM public.uploads u JOIN public.files f ON f.id=u.file_id
            WHERE u.state='cleaning'
            UNION ALL
            SELECT CASE f.state WHEN 'deleted' THEN 'purge' ELSE 'reclaim_cleanup' END,
                CASE f.state WHEN 'deleted' THEN f.deleted_at END
            FROM public.files f JOIN public.locations l ON l.file_id=f.id
            WHERE f.state IN ('deleted','reclaimed')
                AND NOT EXISTS (SELECT 1 FROM public.uploads u WHERE u.file_id=f.id)
         ), queues(queue) AS (VALUES ('expired_pending'),('s3_expired'),('s3_complete'),
             ('native_complete'),('s3_cleanup'),('native_cleanup'),('purge'),('reclaim_cleanup'))
         SELECT q.queue, count(c.queue) AS items, min(c.waiting_at) AS oldest_waiting_at
         FROM queues q LEFT JOIN candidates c USING(queue) GROUP BY q.queue ORDER BY q.queue",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|error| Failure::new("objects", None, error))?;
    let mut retained = Vec::with_capacity(retention.len());
    for (stream, days) in retention {
        retained.push(
            retention::backlog(&mut tx, stream, days, observed_at)
                .await
                .map_err(|error| Failure::new("retention", Some(stream.name()), error))?,
        );
    }
    // Fixed allowlist: no tenant names or arbitrary user tables become labels.
    let tables = sqlx::query_as(
        "SELECT s.schemaname::text AS schema, s.relname::text AS table,
             pg_catalog.pg_table_size(s.relid) AS data_bytes,
             pg_catalog.pg_indexes_size(s.relid) AS index_bytes,
             s.n_live_tup AS estimated_live_rows, s.n_dead_tup AS estimated_dead_rows
         FROM pg_catalog.pg_stat_user_tables s
         WHERE (s.schemaname='public' AND s.relname IN (
             'storages','clients','client_native_keys','client_s3_credentials',
             'files','locations','leases','lease_parts','s3_object_keys','uploads',
             'lease_history','usage_snapshots'))
         OR (s.schemaname='management' AND s.relname IN (
             'accounts','api_tokens','password_credentials','password_setup_tokens',
             'sessions','authentication_budgets','login_budget','audit_events',
             'command_invocations','security_events'))
         ORDER BY s.schemaname,s.relname",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|error| Failure::new("tables", None, error))?;
    tx.commit()
        .await
        .map_err(|error| Failure::new("commit", None, error))?;
    Ok(Snapshot {
        observed_at,
        objects,
        retention: retained,
        tables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_database_failures_keep_only_bounded_context() {
        for (error, reason) in [
            (sqlx::Error::PoolTimedOut, "pool_timeout"),
            (sqlx::Error::PoolClosed, "pool_closed"),
            (
                sqlx::Error::Io(std::io::Error::other("private-connection-details")),
                "connection_failed",
            ),
            (
                sqlx::Error::Decode(Box::new(std::io::Error::other("private-query-value"))),
                "decode_failed",
            ),
        ] {
            let failure = Failure::new("objects", None, error);
            assert_eq!(failure.reason, reason);
            assert_eq!(failure.stage, "objects");
            assert_eq!(failure.stream, None);
            assert!(!format!("{failure:?}").contains("private"));
        }
    }
}
