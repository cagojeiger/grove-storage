#![allow(clippy::unwrap_used)]
mod support {
    #[allow(clippy::panic)]
    pub mod lifecycle;
    pub mod management;
    pub mod time;
}

use chrono::Duration;
use grove_db::{
    PgPool,
    retention::{self, Batch, Stream},
};
use std::num::NonZeroU16;
use support::{
    lifecycle, management,
    time::{base, install_clock, set_time},
};
use uuid::Uuid;

async fn prune(pool: &PgPool, stream: Stream, days: u16, limit: u16) -> Batch {
    retention::prune(
        pool,
        stream,
        NonZeroU16::new(days).unwrap(),
        NonZeroU16::new(limit).unwrap(),
    )
    .await
    .unwrap()
}

async fn account(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO management.accounts(id,display_name,role) VALUES(gen_random_uuid(),'Reader','reader') RETURNING id")
        .fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn session_retention_uses_invalidation_time_and_strict_cutoff(pool: PgPool) {
    install_clock(&pool).await;
    let owner = account(&pool).await;
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,password_generation,created_at,expires_at,revoked_at)
        SELECT gen_random_uuid(),repeat(md5(n::text),2),'password',$1,gen_random_uuid(),'2025-11-01',expires,revoked
        FROM (VALUES
            (1,'2025-12-31T00:00:00Z'::timestamptz,NULL::timestamptz),
            (2,'2025-12-30T23:59:59.999999Z'::timestamptz,NULL::timestamptz),
            (3,'2026-01-02T00:00:00Z'::timestamptz,'2025-12-30T23:59:59Z'::timestamptz),
            (4,'2026-01-02T00:00:00Z'::timestamptz,NULL::timestamptz)) AS v(n,expires,revoked)")
        .bind(owner).execute(&pool).await.unwrap();
    let first = prune(&pool, Stream::Sessions, 1, 1).await;
    assert_eq!(first.deleted, 1);
    assert_eq!(
        first.oldest_remaining_at,
        Some(base() - Duration::days(1) - Duration::microseconds(1))
    );
    assert_eq!(prune(&pool, Stream::Sessions, 1, 10).await.deleted, 1);
    assert_eq!(prune(&pool, Stream::Sessions, 1, 10).await.deleted, 0);
    set_time(&pool, base() + Duration::microseconds(1)).await;
    assert_eq!(prune(&pool, Stream::Sessions, 1, 10).await.deleted, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM management.sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn token_cleanup_preserves_references_and_non_secret_audit_snapshots(pool: PgPool) {
    install_clock(&pool).await;
    let owner = account(&pool).await;
    let secret_hash = management::hash(123);
    let issued = grove_db::management::issue_credential(
        &pool,
        &management::context(),
        owner,
        &grove_db::management::NewCredential {
            label: "CLI reader",
            token_prefix: "gsm_example",
            token_hash: &secret_hash,
            expires_at: base() + Duration::days(90),
        },
    )
    .await
    .unwrap();
    assert!(
        grove_db::management::revoke_credential(&pool, &management::context(), issued.id)
            .await
            .unwrap()
    );
    set_time(&pool, base() + Duration::days(30)).await;
    assert_eq!(prune(&pool, Stream::ApiTokens, 30, 10).await.deleted, 0);
    set_time(
        &pool,
        base() + Duration::days(30) + Duration::microseconds(1),
    )
    .await;
    // A reference must defer deletion rather than cascade a session.
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,credential_id,expires_at)
        VALUES($1,$2,'token',$3,$4,grove_time.wall_now()+interval '1 hour')")
        .bind(session).bind(management::hash(124)).bind(owner).bind(issued.id).execute(&pool).await.unwrap();
    assert_eq!(prune(&pool, Stream::ApiTokens, 30, 10).await.deleted, 0);
    sqlx::query("DELETE FROM management.sessions WHERE id=$1")
        .bind(session)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(prune(&pool, Stream::ApiTokens, 30, 10).await.deleted, 1);
    let history: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata FROM management.audit_events WHERE resource_id=$1 ORDER BY id",
    )
    .bind(issued.id.to_string())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(history.len(), 2);
    for metadata in history {
        assert_eq!(
            metadata.get("label"),
            Some(&serde_json::json!("CLI reader"))
        );
        assert!(metadata.get("token_prefix").is_none());
        assert_eq!(metadata.get("account_id"), Some(&serde_json::json!(owner)));
        assert!(!metadata.to_string().contains(&secret_hash));
        assert!(metadata.get("token_hash").is_none());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn snapshot_failure_rolls_back_token_issuance_and_revocation(pool: PgPool) {
    install_clock(&pool).await;
    let owner = account(&pool).await;
    let hash = management::hash(126);
    let key = grove_db::management::NewCredential {
        label: "Original",
        token_prefix: "gsm_original",
        token_hash: &hash,
        expires_at: base() + Duration::days(90),
    };
    let original =
        grove_db::management::issue_credential(&pool, &management::context(), owner, &key)
            .await
            .unwrap();
    sqlx::raw_sql("CREATE FUNCTION management.reject_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'snapshot failure'; END $$;
        CREATE TRIGGER reject_snapshot BEFORE UPDATE OF metadata ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_snapshot();")
        .execute(&pool).await.unwrap();
    let next_hash = management::hash(127);
    let next = grove_db::management::NewCredential {
        token_hash: &next_hash,
        ..key
    };
    assert!(
        grove_db::management::issue_credential(&pool, &management::context(), owner, &next)
            .await
            .is_err()
    );
    assert!(
        grove_db::management::revoke_credential(&pool, &management::context(), original.id)
            .await
            .is_err()
    );
    assert!(
        grove_db::management::authenticate(&pool, &hash)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        grove_db::management::authenticate(&pool, &next_hash)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM management.audit_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn setup_and_expired_token_cleanup_leave_current_authentication_intact(pool: PgPool) {
    install_clock(&pool).await;
    let one = account(&pool).await;
    let two = account(&pool).await;
    sqlx::query(
        "INSERT INTO management.password_setup_tokens(account_id,token_hash,created_at,expires_at)
        VALUES($1,repeat('a',64),'2025-12-01','2025-12-31'),
              ($2,repeat('b',64),'2025-12-01','2025-12-30T23:59:59.999999Z')",
    )
    .bind(one)
    .bind(two)
    .execute(&pool)
    .await
    .unwrap();
    let active_hash = management::hash(125);
    let active = grove_db::management::issue_credential(
        &pool,
        &management::context(),
        one,
        &grove_db::management::NewCredential {
            label: "Current",
            token_prefix: "gsm_current",
            token_hash: &active_hash,
            expires_at: base() + Duration::days(90),
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO management.api_tokens(id,account_id,label,token_prefix,token_hash,created_at,expires_at)
        VALUES(gen_random_uuid(),$1,'Expired','gsm_old',repeat('c',64),'2025-01-01','2025-12-02')")
        .bind(one).execute(&pool).await.unwrap();
    assert_eq!(prune(&pool, Stream::PasswordSetup, 1, 10).await.deleted, 1);
    assert_eq!(prune(&pool, Stream::ApiTokens, 30, 10).await.deleted, 0);
    set_time(&pool, base() + Duration::microseconds(1)).await;
    assert_eq!(prune(&pool, Stream::PasswordSetup, 1, 10).await.deleted, 1);
    assert_eq!(prune(&pool, Stream::ApiTokens, 30, 10).await.deleted, 1);
    let actor = grove_db::management::authenticate(&pool, &active_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actor.account_id, one);
    assert_eq!(actor.credential_id, Some(active.id));
}

#[sqlx::test(migrations = "./migrations")]
async fn observations_are_independent_oldest_first_and_calendar_bounded(pool: PgPool) {
    install_clock(&pool).await;
    sqlx::raw_sql(
        "INSERT INTO lease_history(at,file_id,storage_id,client_id,kind,size)
        VALUES('2025-10-03',gen_random_uuid(),'removed','removed','read',10),
              ('2025-09-01',gen_random_uuid(),'removed','removed','write',20),
              ('2025-01-01',gen_random_uuid(),'removed','removed','read',30);
        INSERT INTO usage_snapshots(day,storage_id,client_id,active_bytes,active_files)
        VALUES('2024-12-31','removed','removed',1,1),('2025-01-01','removed','removed',1,1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let batch = prune(&pool, Stream::ObjectAccess, 90, 1).await;
    assert_eq!(batch.deleted, 1);
    assert_eq!(
        batch.oldest_remaining_at,
        Some(base() - Duration::days(122))
    );
    assert_eq!(prune(&pool, Stream::ObjectAccess, 90, 10).await.deleted, 1);
    assert_eq!(prune(&pool, Stream::ObjectAccess, 90, 10).await.deleted, 0);
    assert_eq!(prune(&pool, Stream::Usage, 365, 1).await.deleted, 1);
    assert_eq!(prune(&pool, Stream::Usage, 365, 10).await.deleted, 0);
    set_time(&pool, base() + Duration::days(1)).await;
    assert_eq!(prune(&pool, Stream::Usage, 365, 10).await.deleted, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn file_gc_never_discards_native_or_s3_recovery_ownership(pool: PgPool) {
    install_clock(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let native = lifecycle::create_ok(&pool, 10).await;
    let s3 = lifecycle::create_ok(&pool, 10).await;
    sqlx::query(
        "INSERT INTO uploads(file_id,protocol,multipart,state,expected_etag) VALUES($1,'native',true,'completing','expected')",
    )
    .bind(native.file_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO uploads(file_id,protocol,key,multipart) VALUES($1,'s3','key',true)")
        .bind(s3.file_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("UPDATE files SET state='reclaimed'; DELETE FROM locations; DELETE FROM leases;")
        .execute(&pool)
        .await
        .unwrap();
    set_time(&pool, base() + Duration::days(91)).await;
    assert_eq!(
        grove_db::files::prune_terminal_files(&pool, 90 * 86400, 10)
            .await
            .unwrap(),
        0
    );
    sqlx::raw_sql("DELETE FROM uploads;")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        grove_db::files::prune_terminal_files(&pool, 90 * 86400, 10)
            .await
            .unwrap(),
        2
    );
}
