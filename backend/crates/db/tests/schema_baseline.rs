#![allow(clippy::unwrap_used)]

use grove_db::PgPool;
use sqlx::migrate::{Migrate, MigrateError};

#[path = "support/lifecycle.rs"]
#[allow(clippy::panic)]
mod lifecycle;

async fn install_released_baseline(pool: &PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in sqlx::migrate!("./migrations")
        .iter()
        .filter(|m| m.version <= 4)
    {
        connection.apply(migration).await.unwrap();
    }
}

#[sqlx::test(migrations = false)]
async fn released_baseline_upgrades_without_inventing_observation_time(pool: PgPool) {
    install_released_baseline(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    sqlx::query("INSERT INTO usage_snapshots(day,storage_id,client_id,active_bytes,active_files) VALUES('2026-01-01','s','c',10,1)")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO management.accounts(id,display_name,role) VALUES('11111111-1111-1111-1111-111111111111','Reader','reader');
        INSERT INTO management.api_tokens(id,account_id,label,token_prefix,token_hash,expires_at)
        VALUES('22222222-2222-2222-2222-222222222222','11111111-1111-1111-1111-111111111111','Old CLI','gsm_old',repeat('a',64),grove_time.wall_now()+interval '1 day');
        INSERT INTO management.audit_events(actor_kind,request_id,surface,action,resource_type,resource_id)
        VALUES('system',gen_random_uuid(),'cli','credential.issue','credential','22222222-2222-2222-2222-222222222222');")
        .execute(&pool).await.unwrap();
    grove_db::migrate(&pool).await.unwrap();
    grove_db::migrate(&pool).await.unwrap();
    let unchanged: (i64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT active_bytes,observed_at FROM usage_snapshots")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unchanged, (10, None));
    let metadata: serde_json::Value =
        sqlx::query_scalar("SELECT metadata FROM management.audit_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(metadata.get("label"), Some(&serde_json::json!("Old CLI")));
    assert!(metadata.get("token_hash").is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM lease_history WHERE file_id=$1 AND id>0"
        )
        .bind(file.file_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = false)]
async fn invalid_ownership_blocks_upgrade_atomically_and_can_be_repaired(pool: PgPool) {
    install_released_baseline(&pool).await;
    lifecycle::wire(&pool, 100).await;
    grove_db::registry::insert_client(&pool, "other", "s")
        .await
        .unwrap();
    let file = lifecycle::create_ok(&pool, 10).await;
    sqlx::query("INSERT INTO s3_object_keys(client_id,key,file_id) VALUES('other','key',$1)")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(grove_db::migrate(&pool).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        4
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM information_schema.columns WHERE table_name='lease_history' AND column_name='id'")
        .fetch_one(&pool).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM s3_object_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM s3_object_keys WHERE client_id='other'")
        .execute(&pool)
        .await
        .unwrap();
    grove_db::migrate(&pool).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn duplicate_write_owners_block_upgrade_without_erasing_leases(pool: PgPool) {
    install_released_baseline(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    sqlx::query("INSERT INTO leases(file_id,kind,expires_at) VALUES($1,'write',grove_time.transaction_now()+interval '1 hour')")
        .bind(file.file_id).execute(&pool).await.unwrap();
    assert!(grove_db::migrate(&pool).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leases WHERE file_id=$1")
            .bind(file.file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        4
    );
    sqlx::query("UPDATE leases SET state='expired' WHERE id=$1")
        .bind(file.lease_id)
        .execute(&pool)
        .await
        .unwrap();
    grove_db::migrate(&pool).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn fresh_install_creates_only_the_current_schema_and_reapplies_cleanly(pool: PgPool) {
    grove_db::migrate(&pool).await.unwrap();
    grove_db::migrate(&pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(versions, [1, 2, 3, 4, 5, 6, 7, 8]);
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
            "public.s3_object_keys",
            "public.storages",
            "public.uploads",
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
