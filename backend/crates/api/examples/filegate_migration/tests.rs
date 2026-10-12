use super::*;
use sqlx::Connection;

const FILE: &str = "00000000-0000-0000-0000-000000000001";
const ACCOUNT: &str = "00000000-0000-0000-0000-000000000002";

fn test_crypto(label: &str) -> Result<Crypto> {
    Ok(Crypto::new(
        "v1",
        &SecretString::from(format!("migration-fixture-{label}-encryption-root-32bytes")),
    )?)
}

async fn fixture(pool: &PgPool) -> Result<Snapshot> {
    let source = test_crypto("old")?;
    let encrypted = source.encrypt("archive", &SecretString::from("provider-secret".to_owned()))?;
    sqlx::query("INSERT INTO storages(id,kind,endpoint,public_endpoint,region,bucket,force_path_style,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes) VALUES('archive','s3','https://s3.test','https://s3.test','auto','physical',true,'provider-key',$1,$2,'v1',100000)")
        .bind(encrypted.ciphertext).bind(encrypted.nonce).execute(pool).await?;
    sqlx::raw_sql("INSERT INTO clients(id,storage_id) VALUES('consumer','archive'),('other','archive');
        INSERT INTO client_keys(key_hash,client_id) VALUES('sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','consumer');
        INSERT INTO files(id,client_id,state,declared_size,content_type,etag,committed_at) VALUES('00000000-0000-0000-0000-000000000001','consumer','active',11,'text/plain','fixture-etag',now());
        INSERT INTO locations(file_id,storage_id,object_key) VALUES('00000000-0000-0000-0000-000000000001','archive','physical/original-object');
        INSERT INTO s3_keys(client_id,key,file_id) VALUES('consumer','notes/a%2Fb space.txt','00000000-0000-0000-0000-000000000001');
        INSERT INTO leases(file_id,kind,state,expires_at) VALUES('00000000-0000-0000-0000-000000000001','write','committed',now()-interval '1 day');
        INSERT INTO lease_history(file_id,storage_id,client_id,kind,size) SELECT '00000000-0000-0000-0000-000000000001','archive','consumer','write',11 FROM generate_series(1,2);
        INSERT INTO usage_snapshot(day,storage_id,client_id,active_bytes,active_files) VALUES(current_date-1,'archive','consumer',11,1);")
        .execute(pool).await?;
    let encrypted = source.encrypt(
        "consumerkey",
        &SecretString::from("consumer-secret".to_owned()),
    )?;
    sqlx::query("INSERT INTO s3_credentials(access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id) VALUES('consumerkey','consumer',$1,$2,'v1')")
        .bind(encrypted.ciphertext).bind(encrypted.nonce).execute(pool).await?;
    export(pool).await
}

async fn fresh_target(pool: &PgPool) -> Result<PgPool> {
    // This pool is the disposable database owned by sqlx::test.
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(pool)
        .await?;
    grove_db::migrate(pool).await?;
    sqlx::query("INSERT INTO management.accounts(id,display_name,role) VALUES($1::uuid,'Existing administrator','admin')")
        .bind(ACCOUNT).execute(pool).await?;
    let options = pool.connect_options();
    pool.close().await;
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*options).clone())
        .await?)
}

async fn assert_empty_resources(pool: &PgPool) -> Result<()> {
    for (_, table, _) in TABLES {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM public.{table}"))
            .fetch_one(pool)
            .await?;
        ensure!(count == 0, "partial import left rows in {table}");
    }
    let account: String =
        sqlx::query_scalar("SELECT display_name FROM management.accounts WHERE id=$1::uuid")
            .bind(ACCOUNT)
            .fetch_one(pool)
            .await?;
    ensure!(account == "Existing administrator");
    Ok(())
}

#[sqlx::test(migrations = "examples/filegate_migration/legacy")]
async fn preserves_keys_locations_history_and_management_with_new_root(pool: PgPool) -> Result<()> {
    let snapshot = fixture(&pool).await?;
    let pool = fresh_target(&pool).await?;
    let source = test_crypto("old")?;
    let target = test_crypto("new")?;
    import(&pool, &snapshot, &source, &target).await?;
    let mut transaction = pool.begin().await?;
    sqlx::query("SET LOCAL TIME ZONE 'UTC'")
        .execute(&mut *transaction)
        .await?;
    verify(&mut transaction, &snapshot, &source, &target).await?;
    transaction.commit().await?;
    let id: String = sqlx::query_scalar(
        "SELECT file_id::text FROM s3_object_keys WHERE key='notes/a%2Fb space.txt'",
    )
    .fetch_one(&pool)
    .await?;
    ensure!(id == FILE);
    let history: i64 = sqlx::query_scalar("SELECT count(DISTINCT id) FROM lease_history")
        .fetch_one(&pool)
        .await?;
    ensure!(history == 2, "duplicate history events must survive");
    let known_observations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM usage_snapshots WHERE observed_at IS NOT NULL")
            .fetch_one(&pool)
            .await?;
    ensure!(
        known_observations == 0,
        "legacy observation time is unknown"
    );
    let metadata: Value = sqlx::query_scalar("SELECT metadata FROM storages")
        .fetch_one(&pool)
        .await?;
    ensure!(metadata == json!({}));
    let encrypted: Json<Value> =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM client_s3_credentials c")
            .fetch_one(&pool)
            .await?;
    ensure!(decrypt(&target, &encrypted.0, "access_key_id")?.expose_secret() == "consumer-secret");
    ensure!(decrypt(&source, &encrypted.0, "access_key_id").is_err());
    let account: String = sqlx::query_scalar("SELECT display_name FROM management.accounts")
        .fetch_one(&pool)
        .await?;
    ensure!(account == "Existing administrator");
    pool.close().await;
    Ok(())
}

#[sqlx::test(migrations = "examples/filegate_migration/legacy")]
async fn wrong_root_and_late_owner_constraint_roll_back_every_table(pool: PgPool) -> Result<()> {
    let mut snapshot = fixture(&pool).await?;
    let pool = fresh_target(&pool).await?;
    let target = test_crypto("new")?;
    ensure!(
        import(&pool, &snapshot, &test_crypto("wrong")?, &target)
            .await
            .is_err()
    );
    assert_empty_resources(&pool).await?;
    snapshot
        .tables
        .get_mut("s3_keys")
        .and_then(|rows| rows.first_mut())
        .context("keys")?["client_id"] = json!("other");
    let error = import(&pool, &snapshot, &test_crypto("old")?, &target)
        .await
        .err()
        .context("expected FK rejection")?;
    ensure!(error.to_string().contains("23503"));
    ensure!(!error.to_string().contains("provider-secret"));
    assert_empty_resources(&pool).await?;
    pool.close().await;
    Ok(())
}

#[sqlx::test(migrations = "examples/filegate_migration/legacy")]
async fn rejects_repeat_import_and_detects_changed_mapping(pool: PgPool) -> Result<()> {
    let snapshot = fixture(&pool).await?;
    let pool = fresh_target(&pool).await?;
    let source = test_crypto("old")?;
    let target = test_crypto("new")?;
    import(&pool, &snapshot, &source, &target).await?;
    let error = import(&pool, &snapshot, &source, &target)
        .await
        .err()
        .context("expected nonempty target rejection")?;
    ensure!(error.to_string().contains("not empty"));
    sqlx::query("UPDATE locations SET object_key='wrong/object'")
        .execute(&pool)
        .await?;
    let mut connection = pool.acquire().await?;
    ensure!(
        verify(&mut connection, &snapshot, &source, &target)
            .await
            .is_err()
    );
    drop(connection);
    pool.close().await;
    Ok(())
}

#[sqlx::test(migrations = "examples/filegate_migration/legacy")]
async fn rejects_active_connections_and_unsupported_source_history(pool: PgPool) -> Result<()> {
    let snapshot = fixture(&pool).await?;
    sqlx::query("UPDATE _sqlx_migrations SET checksum='\\x00'::bytea WHERE version=1")
        .execute(&pool)
        .await?;
    ensure!(export(&pool).await.is_err());
    let pool = fresh_target(&pool).await?;
    let other = sqlx::PgConnection::connect_with(&pool.connect_options()).await?;
    let error = import(&pool, &snapshot, &test_crypto("old")?, &test_crypto("new")?)
        .await
        .err()
        .context("expected connection rejection")?;
    ensure!(error.to_string().contains("other connections"));
    drop(other);
    assert_empty_resources(&pool).await?;
    pool.close().await;
    Ok(())
}

#[sqlx::test(migrations = "examples/filegate_migration/legacy")]
async fn rejects_pending_leases_fs_and_cleanup_owners(pool: PgPool) -> Result<()> {
    let snapshot = fixture(&pool).await?;
    for (table, field, value) in [
        ("files", "state", "pending"),
        ("leases", "state", "issued"),
        ("storages", "kind", "fs"),
        ("files", "state", "deleted"),
    ] {
        let mut changed = Snapshot {
            format: snapshot.format.clone(),
            captured_at: snapshot.captured_at.clone(),
            tables: snapshot.tables.clone(),
        };
        changed
            .tables
            .get_mut(table)
            .and_then(|rows| rows.first_mut())
            .context("table")?[field] = json!(value);
        ensure!(
            changed.validate().is_err(),
            "unsafe input accepted: {table}/{field}"
        );
    }
    let mut changed = snapshot;
    changed
        .tables
        .get_mut("s3_uploads")
        .context("owners")?
        .push(json!({"file_id": FILE}));
    ensure!(changed.validate().is_err());
    Ok(())
}

#[test]
fn snapshot_file_is_private_and_never_overwrites_an_existing_backup() -> Result<()> {
    let path = std::env::temp_dir().join(format!(
        "grove-migration-test-{}.json",
        uuid::Uuid::new_v4()
    ));
    let snapshot = Snapshot {
        format: "filegate-0.4.1-v1".into(),
        captured_at: "fixture".into(),
        tables: BTreeMap::new(),
    };
    save(&snapshot, &path)?;
    let second = save(&snapshot, &path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(std::fs::metadata(&path)?.permissions().mode() & 0o777 == 0o600);
    }
    std::fs::remove_file(path)?;
    ensure!(second.is_err());
    Ok(())
}
