#![allow(clippy::unwrap_used)]

use grove_db::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn fresh_registry_rejects_filesystem_and_incomplete_s3_settings(pool: PgPool) {
    for statement in [
        "INSERT INTO storages(id,kind,capacity_bytes) VALUES('legacy','fs',100)",
        "INSERT INTO storages(id,capacity_bytes) VALUES('missing-fields',100)",
    ] {
        let error = sqlx::query(statement).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM storages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
