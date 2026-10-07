#![allow(clippy::unwrap_used)]

use grove_db::PgPool;
use sqlx::migrate::{Migrate, MigrateError};

#[sqlx::test(migrations = false)]
async fn fresh_install_creates_only_the_current_schema_and_reapplies_cleanly(pool: PgPool) {
    grove_db::migrate(&pool).await.unwrap();
    grove_db::migrate(&pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(versions, [1, 2, 3, 4]);
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT schemaname || '.' || tablename FROM pg_tables
         WHERE schemaname IN ('public','management') AND tablename <> '_sqlx_migrations'
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "management.accounts",
            "management.api_tokens",
            "management.audit_events",
            "management.authentication_budgets",
            "management.command_invocations",
            "management.login_budget",
            "management.password_credentials",
            "management.password_setup_tokens",
            "management.security_events",
            "management.sessions",
            "public.client_native_keys",
            "public.client_s3_credentials",
            "public.clients",
            "public.files",
            "public.lease_history",
            "public.lease_parts",
            "public.leases",
            "public.locations",
            "public.native_multipart_completions",
            "public.s3_object_keys",
            "public.s3_uploads",
            "public.storages",
            "public.usage_snapshots",
        ]
    );
    let obsolete_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE (table_schema='public' AND table_name='storages' AND column_name='root_path')
            OR (table_schema='management' AND table_name='accounts' AND column_name='kind')
            OR (table_schema='management' AND table_name='sessions'
                AND column_name IN ('user_id','master_generation'))
            OR (table_schema='management' AND column_name='owner_user_id')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(obsolete_columns, 0);
    let direct_clock_defaults: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema IN ('public','management')
         AND table_name <> '_sqlx_migrations'
         AND column_default IN ('now()', 'CURRENT_TIMESTAMP', 'clock_timestamp()')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(direct_clock_defaults, 0);
}

#[sqlx::test(migrations = false)]
async fn different_migration_history_is_rejected_without_changing_existing_rows(pool: PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE existing_data (id integer PRIMARY KEY);
         INSERT INTO existing_data VALUES(1);
         INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time)
         VALUES(1,'old domain schema',true,decode(repeat('00',48),'hex'),0);",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    drop(connection);
    assert!(matches!(
        grove_db::migrate(&pool).await.unwrap_err(),
        MigrateError::VersionMismatch(1)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT id FROM existing_data")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let installed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname='grove_time')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!installed);
}

#[sqlx::test(migrations = false)]
async fn failed_baseline_step_rolls_back_and_can_be_retried(pool: PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < 4)
    {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);
    sqlx::query("CREATE SCHEMA management")
        .execute(&pool)
        .await
        .unwrap();
    assert!(grove_db::migrate(&pool).await.is_err());
    let rolled_back: bool = sqlx::query_scalar("SELECT to_regclass('management.accounts') IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(rolled_back);
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(latest, 3);
    sqlx::query("DROP SCHEMA management")
        .execute(&pool)
        .await
        .unwrap();
    grove_db::migrate(&pool).await.unwrap();
}
