#![allow(clippy::unwrap_used)]

use filegate_db::{PgPool, admin_auth};
use sqlx::migrate::Migrate;

async fn legacy_rows(pool: &PgPool) -> Vec<Vec<String>> {
    let mut snapshot = Vec::new();
    for table in [
        "storages",
        "clients",
        "client_keys",
        "admin_principals",
        "admin_credentials",
        "admin_sessions",
        "admin_audit_events",
    ] {
        let projection = if matches!(table, "storages" | "clients") {
            "(to_jsonb(t)-'metadata')::text"
        } else {
            "to_jsonb(t)::text"
        };
        let rows = sqlx::query_scalar(&format!("SELECT {projection} FROM {table} t ORDER BY 1"))
            .fetch_all(pool)
            .await
            .unwrap();
        snapshot.push(rows);
    }
    snapshot
}

#[sqlx::test(migrations = false)]
async fn additive_upgrade_preserves_existing_registry_credentials_and_sessions(pool: PgPool) {
    let migrations = sqlx::migrate!("./migrations");
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in migrations.iter().filter(|migration| migration.version <= 7) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('existing','fs','/data/existing',100);
        INSERT INTO clients(id,storage_id) VALUES('existing-app','existing');
        INSERT INTO client_keys(key_hash,client_id) VALUES('sha256:' || repeat('a',64),'existing-app');")
        .execute(&pool).await.unwrap();
    let credential = admin_auth::issue(
        &pool,
        admin_auth::IssueMode::Initialize,
        "existing",
        "old-hash",
    )
    .await
    .unwrap()
    .unwrap();
    admin_auth::create_session(&pool, "old-hash", "old-session")
        .await
        .unwrap()
        .unwrap();
    let before = legacy_rows(&pool).await;
    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    assert_eq!(legacy_rows(&pool).await, before);
    for table in ["storages", "clients"] {
        let metadata: serde_json::Value =
            sqlx::query_scalar(&format!("SELECT metadata FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(metadata, serde_json::json!({}));
    }
    assert_eq!(
        admin_auth::session_actor(&pool, "old-session")
            .await
            .unwrap(),
        Some(credential.id)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
