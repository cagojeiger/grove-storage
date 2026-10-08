#![allow(clippy::unwrap_used, clippy::panic)]

use grove_db::{PgPool, files, s3_registry as s3};
use sqlx::Row;
use sqlx::migrate::Migrate;
use uuid::Uuid;

#[path = "support/lifecycle.rs"]
mod lifecycle;

#[path = "support/lock_wait.rs"]
#[allow(clippy::expect_used)]
mod lock_wait;

async fn multipart(pool: &PgPool) -> files::CreatedFile {
    let spec = files::CreateSpec {
        part_size: Some(5),
        ..lifecycle::spec(10)
    };
    match files::create(pool, spec).await.unwrap() {
        files::CreateOutcome::Created(file) => *file,
        files::CreateOutcome::NoClient => panic!("fixture client missing"),
    }
}

async fn native_claim(pool: &PgPool, file_id: Uuid) {
    let files::CompletionStart::Ready(guard) =
        files::begin_completion(pool, file_id).await.unwrap()
    else {
        panic!("expected native completion guard");
    };
    assert!(guard.claim("native-etag", 900).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn native_begin_rechecks_protocol_after_file_lock_wait(pool: PgPool) {
    lifecycle::wire(&pool, 100).await;
    let file = multipart(&pool).await;
    let mut owner = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *owner)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
        .bind(file.file_id)
        .execute(&mut *owner)
        .await
        .unwrap();
    let waiting_pool = pool.clone();
    let waiting = tokio::spawn(async move {
        matches!(
            files::begin_completion(&waiting_pool, file.file_id)
                .await
                .unwrap(),
            files::CompletionStart::Unavailable
        )
    });
    lock_wait::wait_for_blocker(&pool, blocker).await;
    sqlx::query("INSERT INTO uploads(file_id,protocol,key,multipart) VALUES($1,'s3','key',true)")
        .bind(file.file_id)
        .execute(&mut *owner)
        .await
        .unwrap();
    owner.commit().await.unwrap();
    assert!(waiting.await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_file_has_one_durable_owner_across_protocols(pool: PgPool) {
    lifecycle::wire(&pool, 100).await;
    let file = multipart(&pool).await;
    native_claim(&pool, file.file_id).await;
    let error = sqlx::query(
        "INSERT INTO uploads(file_id,protocol,key,multipart) VALUES($1,'s3','key',true)",
    )
    .bind(file.file_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    sqlx::query("DELETE FROM uploads WHERE file_id=$1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO uploads(file_id,protocol,key,multipart) VALUES($1,'s3','key',true)")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let error = sqlx::query("INSERT INTO uploads(file_id,protocol,multipart,state,expected_etag) VALUES($1,'native',true,'completing','etag')")
        .bind(file.file_id).execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn protocol_shapes_are_enforced_by_the_database(pool: PgPool) {
    lifecycle::wire(&pool, 100).await;
    let file = multipart(&pool).await;
    for values in [
        "'native',NULL,true,false,'open',NULL,'etag'",
        "'native','key',true,false,'completing',NULL,'etag'",
        "'native',NULL,false,false,'completing',NULL,'etag'",
        "'native',NULL,true,true,'completing',NULL,'etag'",
        "'native',NULL,true,false,'completing',10,'etag'",
        "'native',NULL,true,false,'cleaning',NULL,NULL",
        "'s3',NULL,true,false,'open',NULL,NULL",
        "'s3','key',true,false,'completing',NULL,'etag'",
        "'s3','key',true,false,'completing',10,NULL",
        "'s3','key',true,false,'open',10,'etag'",
        "'s3','key',true,false,'cleaning',10,'etag'",
        "'other','key',true,false,'open',NULL,NULL",
    ] {
        let error = sqlx::query(&format!("INSERT INTO uploads(file_id,protocol,key,multipart,if_none_match,state,expected_size,expected_etag) VALUES($1,{values})"))
            .bind(file.file_id).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514"),
            "{values}"
        );
    }
    sqlx::query("INSERT INTO uploads(file_id,protocol,multipart,state,expected_etag) VALUES($1,'native',true,'cleaning','etag')")
        .bind(file.file_id).execute(&pool).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn protocol_adapters_cannot_finalize_or_recover_each_others_uploads(pool: PgPool) {
    lifecycle::wire(&pool, 100).await;
    let native = multipart(&pool).await;
    native_claim(&pool, native.file_id).await;
    let spec = files::CreateSpec {
        part_size: Some(5),
        ..lifecycle::spec(10)
    };
    let files::CreateOutcome::Created(s3_file) =
        s3::create_upload(&pool, spec, "key", false).await.unwrap()
    else {
        panic!("expected s3 upload");
    };
    // S3 UploadPart still uses this shared renewal path while open.
    assert!(
        files::extend_write_lease(&pool, s3_file.lease_id, 900)
            .await
            .unwrap()
    );
    assert_eq!(
        s3::claim_completion(
            &pool,
            s3::CompletionSpec {
                client_id: "c",
                key: "key",
                file_id: s3_file.file_id,
                multipart: true,
                expected_size: 10,
                expected_etag: "s3-etag",
                lease_ttl_secs: 900,
            }
        )
        .await
        .unwrap(),
        s3::CompletionClaim::Claimed
    );
    assert!(
        !s3::renew_completion_lease(&pool, native.file_id, 900)
            .await
            .unwrap()
    );
    assert!(
        !files::renew_completion_lease(&pool, s3_file.file_id, 900)
            .await
            .unwrap()
    );
    assert!(
        !files::finalize_completion(&pool, s3_file.file_id, "s3-etag")
            .await
            .unwrap()
    );
    assert_eq!(
        s3::finalize_multipart_upload(&pool, "c", "key", native.file_id)
            .await
            .unwrap(),
        s3::FinalizeOutcome::NotPending
    );
    sqlx::query("UPDATE leases SET expires_at=grove_time.transaction_now()-interval '1 hour'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !s3::reopen_completion(&pool, native.file_id, 900)
            .await
            .unwrap()
    );
    assert!(
        !s3::mark_completion_aborting(&pool, native.file_id)
            .await
            .unwrap()
    );
    assert!(
        !files::reopen_completion(&pool, s3_file.file_id, 900)
            .await
            .unwrap()
    );
    assert!(!files::claim_cleanup(&pool, s3_file.file_id).await.unwrap());
    let native_candidates = files::completion_candidates(&pool, 10).await.unwrap();
    assert_eq!(
        native_candidates
            .iter()
            .map(|c| c.file_id)
            .collect::<Vec<_>>(),
        [native.file_id]
    );
    let s3_candidates = s3::completion_candidates(&pool, 10).await.unwrap();
    assert_eq!(
        s3_candidates.iter().map(|c| c.file_id).collect::<Vec<_>>(),
        [s3_file.file_id]
    );
    assert!(files::claim_cleanup(&pool, native.file_id).await.unwrap());
    assert!(
        s3::mark_completion_aborting(&pool, s3_file.file_id)
            .await
            .unwrap()
    );
    assert!(!s3::finalize_abort(&pool, native.file_id).await.unwrap());
    assert!(
        !files::finalize_completion_cleanup(&pool, s3_file.file_id)
            .await
            .unwrap()
    );
    assert_eq!(
        files::completion_cleanup_candidates(&pool, 10)
            .await
            .unwrap()
            .iter()
            .map(|c| c.file_id)
            .collect::<Vec<_>>(),
        [native.file_id]
    );
    assert_eq!(
        s3::cleanup_candidates(&pool, 10)
            .await
            .unwrap()
            .iter()
            .map(|c| c.file_id)
            .collect::<Vec<_>>(),
        [s3_file.file_id]
    );
    assert!(
        files::finalize_completion_cleanup(&pool, native.file_id)
            .await
            .unwrap()
    );
    assert!(s3::finalize_abort(&pool, s3_file.file_id).await.unwrap());
}

async fn install_previous_schema(pool: &PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in sqlx::migrate!("./migrations")
        .iter()
        .filter(|m| m.version <= 5)
    {
        connection.apply(migration).await.unwrap();
    }
}

#[sqlx::test(migrations = false)]
async fn upgrade_preserves_protocol_state_and_recovery_material(pool: PgPool) {
    install_previous_schema(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let mut expected = Vec::new();
    for (protocol, old_state, new_state) in [
        ("native", "completing", "completing"),
        ("native", "cleaning", "cleaning"),
        ("s3", "open", "open"),
        ("s3", "completing", "completing"),
        ("s3", "aborting", "cleaning"),
    ] {
        let file = multipart(&pool).await;
        if protocol == "native" {
            sqlx::query("INSERT INTO native_multipart_completions(file_id,state,expected_etag,created_at,updated_at) VALUES($1,$2,'etag','2026-01-01','2026-01-02')")
                .bind(file.file_id).bind(old_state).execute(&pool).await.unwrap();
        } else {
            sqlx::query("INSERT INTO s3_uploads(file_id,key,multipart,if_none_match,state,expected_size,expected_etag,created_at,updated_at) VALUES($1,'key',true,true,$2,CASE WHEN $2='completing' THEN 10 END,CASE WHEN $2='completing' THEN 'etag' END,'2026-01-01','2026-01-02')")
                .bind(file.file_id).bind(old_state).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE leases SET upload_id='vendor-upload' WHERE id=$1")
            .bind(file.lease_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO lease_parts(lease_id,part_no,state,uploaded_size,uploaded_md5) VALUES($1,1,'done',5,'part-etag')").bind(file.lease_id).execute(&pool).await.unwrap();
        expected.push((file.file_id, protocol, new_state));
    }
    grove_db::migrate(&pool).await.unwrap();
    for (file_id, protocol, state) in expected {
        let row = sqlx::query(
            "SELECT u.*, le.upload_id, lp.uploaded_size, lp.uploaded_md5
             FROM uploads u JOIN leases le ON le.file_id=u.file_id
             JOIN lease_parts lp ON lp.lease_id=le.id WHERE u.file_id=$1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("protocol"), protocol);
        assert_eq!(row.get::<String, _>("state"), state);
        assert!(row.get::<bool, _>("multipart"));
        assert_eq!(row.get::<bool, _>("if_none_match"), protocol == "s3");
        assert_eq!(
            row.get::<Option<String>, _>("key").as_deref(),
            (protocol == "s3").then_some("key")
        );
        assert_eq!(
            row.get::<Option<i64>, _>("expected_size"),
            (protocol == "s3" && state == "completing").then_some(10)
        );
        assert_eq!(
            row.get::<Option<String>, _>("expected_etag").as_deref(),
            (protocol == "native" || state == "completing").then_some("etag")
        );
        for (column, timestamp) in [
            ("created_at", "2026-01-01T00:00:00Z"),
            ("updated_at", "2026-01-02T00:00:00Z"),
        ] {
            assert_eq!(
                row.get::<chrono::DateTime<chrono::Utc>, _>(column),
                timestamp.parse::<chrono::DateTime<chrono::Utc>>().unwrap()
            );
        }
        assert_eq!(row.get::<String, _>("upload_id"), "vendor-upload");
        assert_eq!(row.get::<i64, _>("uploaded_size"), 5);
        assert_eq!(row.get::<String, _>("uploaded_md5"), "part-etag");
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM uploads")
            .fetch_one(&pool)
            .await
            .unwrap(),
        5
    );
    let retired: bool = sqlx::query_scalar("SELECT to_regclass('s3_uploads') IS NULL AND to_regclass('native_multipart_completions') IS NULL").fetch_one(&pool).await.unwrap();
    assert!(retired);
}

#[sqlx::test(migrations = false)]
async fn conflicting_legacy_owners_reject_upgrade_atomically(pool: PgPool) {
    install_previous_schema(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let file = multipart(&pool).await;
    sqlx::query(
        "INSERT INTO native_multipart_completions(file_id,expected_etag) VALUES($1,'etag')",
    )
    .bind(file.file_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO s3_uploads(file_id,key,multipart) VALUES($1,'key',true)")
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
        5
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('uploads') IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT (SELECT count(*) FROM s3_uploads)+(SELECT count(*) FROM native_multipart_completions)").fetch_one(&pool).await.unwrap(), 2);
    sqlx::query("DELETE FROM s3_uploads WHERE file_id=$1")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
    grove_db::migrate(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT protocol FROM uploads WHERE file_id=$1")
            .bind(file.file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "native"
    );
}
