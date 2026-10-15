#![allow(clippy::unwrap_used)]
mod support {
    #[allow(clippy::panic)]
    pub mod lifecycle;
    pub mod time;
}

use chrono::Duration;
use grove_db::{
    PgPool,
    observations::{self, Snapshot},
    retention,
};
use std::num::NonZeroU16;
use support::{
    lifecycle,
    time::{base, install_clock, set_time},
};
use uuid::Uuid;

fn plan() -> [(retention::Stream, NonZeroU16); 8] {
    use retention::Stream;
    [
        (Stream::Audit, 365),
        (Stream::Security, 90),
        (Stream::Invocations, 90),
        (Stream::Sessions, 1),
        (Stream::PasswordSetup, 1),
        (Stream::ApiTokens, 30),
        (Stream::ObjectAccess, 90),
        (Stream::Usage, 365),
    ]
    .map(|(stream, days)| (stream, NonZeroU16::new(days).unwrap()))
}

async fn snapshot(pool: &PgPool) -> Snapshot {
    observations::collect(pool, plan()).await.unwrap()
}

fn backlog<'a>(snapshot: &'a Snapshot, queue: &str) -> &'a observations::ObjectBacklog {
    snapshot
        .objects
        .iter()
        .find(|row| row.queue == queue)
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_queues_and_tables_are_bounded_and_sizes_are_not_row_counts(pool: PgPool) {
    install_clock(&pool).await;
    sqlx::raw_sql(
        "CREATE TABLE public.private_tenant_name(value text);
        INSERT INTO management.security_events(actor_kind,request_id,surface,event_type,reason_code)
        VALUES('anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated');
        ANALYZE management.security_events;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let snapshot = snapshot(&pool).await;
    assert_eq!(snapshot.observed_at, base());
    assert_eq!(snapshot.objects.len(), 8);
    assert!(
        snapshot
            .objects
            .iter()
            .all(|row| row.items == 0 && row.oldest_waiting_at.is_none())
    );
    assert_eq!(snapshot.retention.len(), 8);
    assert!(
        snapshot
            .retention
            .iter()
            .all(|row| row.rows == 0 && row.oldest_at.is_none())
    );
    assert_eq!(snapshot.tables.len(), 22);
    assert!(
        !snapshot
            .tables
            .iter()
            .any(|row| row.table.contains("private") || row.schema == "grove_time")
    );
    let security = snapshot
        .tables
        .iter()
        .find(|row| row.schema == "management" && row.table == "security_events")
        .unwrap();
    assert!(security.data_bytes > 0);
    assert!(security.index_bytes > 0);
    assert!(security.estimated_live_rows >= 0);
    assert!(security.estimated_dead_rows >= 0);
    // Collection never prunes or changes the transaction settings of a pooled connection.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM management.security_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("INSERT INTO public.private_tenant_name VALUES ('still writable')")
        .execute(&pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn queues_include_backoff_but_exclude_live_uploads_and_finished_cleanup(pool: PgPool) {
    install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    let cases = [
        "expired_pending",
        "s3_expired",
        "s3_complete",
        "native_complete",
        "s3_cleanup",
        "native_cleanup",
        "purge",
        "reclaim_cleanup",
    ];
    for queue in cases {
        let file = lifecycle::create_ok(&pool, 1).await;
        sqlx::query("UPDATE leases SET expires_at=$2 WHERE file_id=$1")
            .bind(file.file_id)
            .bind(base() - Duration::microseconds(1))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE files SET recovery_after=$2 WHERE id=$1")
            .bind(file.file_id)
            .bind(base() + Duration::hours(1))
            .execute(&pool)
            .await
            .unwrap();
        match queue {
            "purge" => {
                sqlx::query(
                    "UPDATE files SET state='deleted',committed_at=$2,deleted_at=$2 WHERE id=$1",
                )
                .bind(file.file_id)
                .bind(base())
                .execute(&pool)
                .await
                .unwrap();
            }
            "reclaim_cleanup" => {
                sqlx::query("UPDATE files SET state='reclaimed' WHERE id=$1")
                    .bind(file.file_id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "expired_pending" => {}
            _ => {
                let protocol = if queue.starts_with("native") {
                    "native"
                } else {
                    "s3"
                };
                let state = if queue.ends_with("cleanup") {
                    "cleaning"
                } else if queue.ends_with("complete") {
                    "completing"
                } else {
                    "open"
                };
                sqlx::query("INSERT INTO uploads(file_id,protocol,key,multipart,state,expected_size,expected_etag,recovery_after)
                    VALUES($1,$2,CASE WHEN $2='s3' THEN 'private-key' END,true,$3,
                        CASE WHEN $2='s3' AND $3='completing' THEN 1 END,
                        CASE WHEN $2='native' OR $3='completing' THEN 'etag' END,$4)")
                    .bind(file.file_id).bind(protocol).bind(state).bind(base()+Duration::hours(1)).execute(&pool).await.unwrap();
            }
        }
    }
    let live = lifecycle::create_ok(&pool, 1).await;
    // Exact expiry is still live under the reconciler's strict comparison.
    sqlx::query("UPDATE leases SET expires_at=$2 WHERE file_id=$1")
        .bind(live.file_id)
        .bind(base())
        .execute(&pool)
        .await
        .unwrap();
    let finished = lifecycle::create_ok(&pool, 1).await;
    sqlx::query("UPDATE files SET state='reclaimed' WHERE id=$1")
        .bind(finished.file_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM locations WHERE file_id=$1")
        .bind(finished.file_id)
        .execute(&pool)
        .await
        .unwrap();
    let first = snapshot(&pool).await;
    for queue in cases {
        let row = backlog(&first, queue);
        assert_eq!(row.items, 1, "{queue}");
        let expected = match queue {
            "reclaim_cleanup" => None,
            "expired_pending" | "s3_expired" | "s3_complete" | "native_complete" => {
                Some(base() - Duration::microseconds(1))
            }
            _ => Some(base()),
        };
        assert_eq!(row.oldest_waiting_at, expected, "{queue}");
    }
    set_time(&pool, base() + Duration::microseconds(1)).await;
    assert_eq!(backlog(&snapshot(&pool).await, "expired_pending").items, 2);
    sqlx::query("DELETE FROM locations WHERE file_id IN (SELECT id FROM files WHERE state IN ('deleted','reclaimed'))").execute(&pool).await.unwrap();
    let last = snapshot(&pool).await;
    assert_eq!(backlog(&last, "purge").items, 0);
    assert!(backlog(&last, "purge").oldest_waiting_at.is_none());
    assert_eq!(backlog(&last, "reclaim_cleanup").items, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn retention_uses_the_same_strict_policy_and_reference_guards_as_pruning(pool: PgPool) {
    install_clock(&pool).await;
    sqlx::raw_sql("INSERT INTO management.security_events(created_at,actor_kind,request_id,surface,event_type,reason_code)
        VALUES('2025-10-03','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated'),
              ('2025-10-02T23:59:59.999999Z','anonymous',gen_random_uuid(),'console','authentication_failed','unauthenticated');
        INSERT INTO usage_snapshots(day,storage_id,client_id,active_bytes,active_files)
        VALUES('2024-12-31','private','private',1,1),('2025-01-01','private','private',1,1);")
        .execute(&pool).await.unwrap();
    let account = Uuid::new_v4();
    let token = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO management.accounts(id,display_name,role) VALUES($1,'Private','reader')",
    )
    .bind(account)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO management.api_tokens(id,account_id,label,token_prefix,token_hash,created_at,expires_at)
        VALUES($1,$2,'Private','gsm_private',repeat('a',64),'2025-01-01','2025-01-02')").bind(token).bind(account).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,credential_id,expires_at)
        VALUES(gen_random_uuid(),repeat('b',64),'token',$1,$2,'2026-02-01')").bind(account).bind(token).execute(&pool).await.unwrap();
    let first = snapshot(&pool).await;
    let row = |name| {
        first
            .retention
            .iter()
            .find(|row| row.stream == name)
            .unwrap()
    };
    assert_eq!(row("security_events").rows, 1);
    assert_eq!(
        row("security_events").oldest_at,
        Some(base() - Duration::days(90) - Duration::microseconds(1))
    );
    assert_eq!(row("usage_snapshots").rows, 1);
    assert_eq!(row("api_tokens").rows, 0);
    retention::prune(
        &pool,
        retention::Stream::Security,
        NonZeroU16::new(90).unwrap(),
        NonZeroU16::new(10).unwrap(),
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM management.sessions")
        .execute(&pool)
        .await
        .unwrap();
    let after = snapshot(&pool).await;
    assert_eq!(
        after
            .retention
            .iter()
            .find(|r| r.stream == "security_events")
            .unwrap()
            .rows,
        0
    );
    assert_eq!(
        after
            .retention
            .iter()
            .find(|r| r.stream == "api_tokens")
            .unwrap()
            .rows,
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn blocked_collection_fails_with_a_bounded_lock_wait_and_can_retry(pool: PgPool) {
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE public.files IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let error = observations::collect(&pool, plan()).await.unwrap_err();
    assert_eq!(error.stage, "objects");
    assert_eq!(error.stream, None);
    assert_eq!(error.reason, "lock_timeout");
    lock.rollback().await.unwrap();
    assert_eq!(snapshot(&pool).await.objects.len(), 8);
}

#[sqlx::test(migrations = "./migrations")]
async fn collection_enforces_read_only_transactions(pool: PgPool) {
    sqlx::raw_sql(
        "CREATE TABLE public.observation_write_attempt(value boolean);
        CREATE OR REPLACE FUNCTION grove_time.transaction_now() RETURNS timestamptz
        LANGUAGE plpgsql VOLATILE AS $$ BEGIN
            INSERT INTO public.observation_write_attempt VALUES(true);
            RETURN CURRENT_TIMESTAMP;
        END $$;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let error = observations::collect(&pool, plan()).await.unwrap_err();
    assert_eq!(error.stage, "clock");
    assert_eq!(error.reason, "read_only_violation");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.observation_write_attempt")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn failure_context_identifies_retention_stream_without_sql_or_values(pool: PgPool) {
    sqlx::query("ALTER TABLE management.security_events RENAME TO private_table_name")
        .execute(&pool)
        .await
        .unwrap();
    let error = observations::collect(&pool, plan()).await.unwrap_err();
    assert_eq!(error.stage, "retention");
    assert_eq!(error.stream, Some("security_events"));
    assert_eq!(error.reason, "schema_mismatch");
    let safe = format!("{error:?}");
    assert!(!safe.contains("private"));
    assert!(!safe.contains("SELECT"));
}

#[sqlx::test(migrations = "./migrations")]
async fn database_error_reasons_are_bounded_and_do_not_retain_messages(pool: PgPool) {
    for (code, reason) in [
        ("42501", "permission_denied"),
        ("57014", "query_canceled"),
        ("08006", "connection_failed"),
        ("P0001", "database_error"),
    ] {
        sqlx::raw_sql(&format!(
            "CREATE OR REPLACE FUNCTION grove_time.transaction_now()
            RETURNS timestamptz LANGUAGE plpgsql VOLATILE AS $$ BEGIN
            RAISE EXCEPTION USING ERRCODE='{code}', MESSAGE='private-query-value-and-secret';
            END $$;"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let error = observations::collect(&pool, plan()).await.unwrap_err();
        assert_eq!(error.stage, "clock");
        assert_eq!(error.stream, None);
        assert_eq!(error.reason, reason);
        assert!(!format!("{error:?}").contains("private"));
    }
}
