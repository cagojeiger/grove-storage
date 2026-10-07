#![allow(clippy::unwrap_used)]
use chrono::{DateTime, Utc};
use grove_db::PgPool;

pub fn base() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

// SQLx owns this disposable database. Production has no clock override.
pub async fn install_clock(pool: &PgPool) {
    // A history maps PostgreSQL transaction start to the virtual clock version.
    // Advancing wall time must not change a transaction already in progress.
    sqlx::raw_sql("CREATE TABLE grove_time.test_clock(
        id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
        at timestamptz NOT NULL, real_at timestamptz NOT NULL DEFAULT clock_timestamp());
        INSERT INTO grove_time.test_clock(at) VALUES ('2026-01-01T00:00:00Z');
        CREATE OR REPLACE FUNCTION grove_time.wall_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT at FROM grove_time.test_clock ORDER BY id DESC LIMIT 1 $$;
        CREATE OR REPLACE FUNCTION grove_time.transaction_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT at FROM grove_time.test_clock WHERE real_at<=CURRENT_TIMESTAMP ORDER BY id DESC LIMIT 1 $$;")
        .execute(pool).await.unwrap();
}

pub async fn set_time(pool: &PgPool, at: DateTime<Utc>) {
    sqlx::query("INSERT INTO grove_time.test_clock(at) VALUES($1)")
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
}
