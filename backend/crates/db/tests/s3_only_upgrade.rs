#![allow(clippy::unwrap_used)]

use filegate_db::{PgPool, registry};
use serde_json::Value;
use sqlx::migrate::Migrate;

async fn old_schema(pool: &PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in sqlx::migrate!("./migrations")
        .iter()
        .filter(|m| m.version <= 16)
    {
        connection.apply(migration).await.unwrap();
    }
}

async fn snapshot(pool: &PgPool) -> Vec<Vec<Value>> {
    let mut result = Vec::new();
    for table in [
        "storages",
        "clients",
        "client_keys",
        "s3_credentials",
        "files",
        "locations",
        "leases",
        "lease_parts",
        "s3_keys",
        "s3_uploads",
        "native_multipart_completions",
        "lease_history",
        "usage_snapshot",
        "management.accounts",
        "management.credentials",
        "management.sessions",
        "management.audit_events",
    ] {
        let projection = if table == "storages" {
            "to_jsonb(t)-'root_path'"
        } else {
            "to_jsonb(t)"
        };
        result.push(
            sqlx::query_scalar(&format!("SELECT {projection} FROM {table} t ORDER BY 1"))
                .fetch_all(pool)
                .await
                .unwrap(),
        );
    }
    result
}

#[sqlx::test(migrations = false)]
async fn s3_upgrade_preserves_resource_rows_and_management_identity(pool: PgPool) {
    old_schema(&pool).await;
    sqlx::raw_sql("INSERT INTO storages(id,kind,force_relay,endpoint,public_endpoint,region,bucket,force_path_style,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes,metadata)
        VALUES('s','s3',true,'https://internal.example','https://public.example','region','physical-bucket',true,'access',decode('abcd','hex'),decode(repeat('01',12),'hex'),'v1',1000,'{\"owner\":\"home\"}');
        INSERT INTO clients(id,storage_id,metadata) VALUES('c','s','{\"service\":\"notegate\"}');
        INSERT INTO client_keys(client_id,key_hash) VALUES('c','sha256:'||repeat('a',64));
        INSERT INTO s3_credentials(access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id)
        VALUES('servicekey','c',decode('bcde','hex'),decode(repeat('02',12),'hex'),'v1');
        INSERT INTO files(id,client_id,state,declared_size,etag,committed_at,part_size) VALUES
          ('00000000-0000-0000-0000-000000000001','c','active',10,'etag',now(),NULL),
          ('00000000-0000-0000-0000-000000000002','c','pending',20,NULL,NULL,10),
          ('00000000-0000-0000-0000-000000000003','c','pending',30,NULL,NULL,10);
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'s','unchanged/'||id::text FROM files;
        INSERT INTO leases(id,file_id,kind,state,expires_at,upload_id)
          SELECT id,id,'write',CASE WHEN state='active' THEN 'committed' ELSE 'issued' END,now()+interval '1 hour','vendor-'||id::text FROM files;
        INSERT INTO lease_parts(lease_id,part_no,state,uploaded_size,uploaded_md5)
          VALUES('00000000-0000-0000-0000-000000000002',1,'done',10,'checksum');
        INSERT INTO s3_keys(client_id,key,file_id) VALUES('c','logical/key','00000000-0000-0000-0000-000000000001');
        INSERT INTO s3_uploads(file_id,key,multipart,state,expected_size,expected_etag)
          VALUES('00000000-0000-0000-0000-000000000002','pending/key',true,'completing',20,'expected');
        INSERT INTO native_multipart_completions(file_id,expected_etag)
          VALUES('00000000-0000-0000-0000-000000000003','native-etag');
        INSERT INTO lease_history(file_id,storage_id,client_id,kind,size)
          SELECT id,'s','c','write',declared_size FROM files;
        INSERT INTO usage_snapshot(day,storage_id,client_id,active_bytes,active_files) VALUES(current_date,'s','c',10,1);")
        .execute(&pool).await.unwrap();
    let context = filegate_db::management::AuditContext {
        actor: filegate_db::management::AuditActor::Master { session_id: None },
        request_id: uuid::Uuid::new_v4(),
        surface: grove_management_policy::Surface::Console,
    };
    let hash = "a".repeat(64);
    let (account, _) = filegate_db::management::bootstrap(
        &pool,
        &context,
        "Owner",
        &filegate_db::management::NewCredential {
            label: "kept",
            token_prefix: "gst_test",
            token_hash: &hash,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        },
    )
    .await
    .unwrap();
    let before = snapshot(&pool).await;
    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
    let root_column: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='storages' AND column_name='root_path')")
        .fetch_one(&pool).await.unwrap();
    assert!(!root_column);
    assert_eq!(
        registry::client_storage(&pool, "c")
            .await
            .unwrap()
            .as_deref(),
        Some("s")
    );
    assert!(
        registry::client_id_for_key_hash(&pool, &format!("sha256:{hash}"))
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        filegate_db::management::authenticate(&pool, &hash)
            .await
            .unwrap()
            .unwrap()
            .account_id,
        account
    );
    assert_eq!(
        registry::get_storage(&pool, "s")
            .await
            .unwrap()
            .unwrap()
            .bucket
            .as_deref(),
        Some("physical-bucket")
    );
}

#[sqlx::test(migrations = false)]
async fn filesystem_rows_block_upgrade_without_losing_data(pool: PgPool) {
    old_schema(&pool).await;
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('legacy','fs','/data/keep',100);
        INSERT INTO clients(id,storage_id) VALUES('c','legacy');
        WITH f AS (INSERT INTO files(client_id,declared_size) VALUES('c',10) RETURNING id)
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'legacy','keep-this-key' FROM f;")
        .execute(&pool).await.unwrap();
    let before = snapshot(&pool).await;
    let error = filegate_db::migrate(&pool).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("filesystem storages to be migrated first")
    );
    assert_eq!(snapshot(&pool).await, before);
    let path: String = sqlx::query_scalar("SELECT root_path FROM storages WHERE id='legacy'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(path, "/data/keep");
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(latest, 16);
}
