//! PostgreSQL remains the time authority for shared lifecycle state.
pub async fn now(pool: &sqlx::PgPool) -> Result<chrono::DateTime<chrono::Utc>, sqlx::Error> {
    sqlx::query_scalar("SELECT grove_time.wall_now()")
        .fetch_one(pool)
        .await
}
