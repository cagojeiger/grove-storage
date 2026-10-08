#![allow(clippy::unwrap_used, clippy::panic)]
#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/management.rs"]
mod support;
#[path = "support/time.rs"]
mod time;

use chrono::{DateTime, Duration, Utc};
use grove_db::{
    PgPool,
    management::{self as db, admission},
};
use support::*;
use time::{base, install_clock, set_time};
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn session_and_parent_token_expire_at_the_exact_boundary(pool: PgPool) {
    install_clock(&pool).await;
    let (owner, _) = bootstrap(&pool).await;
    let token = hash(20);
    let mut spec = key(&token);
    spec.expires_at = base() + Duration::days(1);
    db::issue_credential(&pool, &context(), owner, &spec)
        .await
        .unwrap();
    let session_hash = hash(21);
    let session = db::create_session(&pool, Uuid::new_v4(), &token, &session_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.expires_at, base() + Duration::hours(8));
    let created: DateTime<Utc> =
        sqlx::query_scalar("SELECT created_at FROM management.sessions WHERE id=$1")
            .bind(session.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(created, base());
    set_time(&pool, session.expires_at - Duration::microseconds(1)).await;
    assert!(
        db::session_actor(&pool, &session_hash)
            .await
            .unwrap()
            .is_some()
    );
    set_time(&pool, session.expires_at).await;
    assert!(
        db::session_actor(&pool, &session_hash)
            .await
            .unwrap()
            .is_none()
    );
    set_time(&pool, spec.expires_at - Duration::microseconds(1)).await;
    assert!(db::authenticate(&pool, &token).await.unwrap().is_some());
    set_time(&pool, spec.expires_at).await;
    assert!(db::authenticate(&pool, &token).await.unwrap().is_none());
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &token, &hash(22))
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn login_window_resets_at_exactly_one_minute(pool: PgPool) {
    install_clock(&pool).await;
    let (owner, _) = bootstrap(&pool).await;
    for _ in 0..60 {
        assert!(
            admission::account_allowed(&pool, owner, admission::Purpose::Login)
                .await
                .unwrap()
        );
    }
    set_time(
        &pool,
        base() + Duration::minutes(1) - Duration::microseconds(1),
    )
    .await;
    assert!(
        !admission::account_allowed(&pool, owner, admission::Purpose::Login)
            .await
            .unwrap()
    );
    set_time(&pool, base() + Duration::minutes(1)).await;
    assert!(
        admission::account_allowed(&pool, owner, admission::Purpose::Login)
            .await
            .unwrap()
    );
    let (attempts, start): (i32, DateTime<Utc>) = sqlx::query_as("SELECT attempts,window_start FROM management.authentication_budgets WHERE account_id=$1 AND purpose='login'").bind(owner).fetch_one(&pool).await.unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(start, base() + Duration::minutes(1));
}

#[sqlx::test(migrations = "./migrations")]
async fn retention_uses_injected_database_time_across_connections(pool: PgPool) {
    use grove_db::retention::{Stream, prune};
    use std::num::NonZeroU16;
    install_clock(&pool).await;
    sqlx::query("INSERT INTO management.security_events(actor_kind,request_id,surface,event_type,reason_code) VALUES('anonymous',$1,'console','authentication_failed','unauthenticated')").bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let days = NonZeroU16::new(90).unwrap();
    let limit = NonZeroU16::new(10).unwrap();
    set_time(&pool, base() + Duration::days(90)).await;
    assert_eq!(
        prune(&pool, Stream::Security, days, limit)
            .await
            .unwrap()
            .deleted,
        0
    );
    set_time(
        &pool,
        base() + Duration::days(90) + Duration::microseconds(1),
    )
    .await;
    assert_eq!(
        prune(&pool, Stream::Security, days, limit)
            .await
            .unwrap()
            .deleted,
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_write_cannot_renew_and_reclaim_rechecks_the_current_time(pool: PgPool) {
    use grove_db::files;
    install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 100).await;
    assert!(file.object_key.starts_with("fg/c/2026/01/"));
    let expires = base() + Duration::seconds(900);
    set_time(&pool, expires - Duration::microseconds(1)).await;
    assert!(files::expired_pending(&pool, 20).await.unwrap().is_empty());
    set_time(&pool, expires).await;
    assert!(
        !files::extend_write_lease(&pool, file.lease_id, 900)
            .await
            .unwrap()
    );
    assert!(files::expired_pending(&pool, 20).await.unwrap().is_empty());
    set_time(&pool, expires + Duration::microseconds(1)).await;
    let candidates = files::expired_pending(&pool, 20).await.unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = candidates.first().unwrap();
    // Model a competing renewal's committed result after the scan. The expired
    // caller itself cannot renew; finalize must independently check the row.
    sqlx::query("UPDATE leases SET expires_at=$2 WHERE id=$1")
        .bind(file.lease_id)
        .bind(expires + Duration::seconds(900))
        .execute(&pool)
        .await
        .unwrap();
    assert!(!files::finalize_reclaim(&pool, candidate).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn transaction_time_stays_frozen_when_wall_time_advances(pool: PgPool) {
    install_clock(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let before: DateTime<Utc> = sqlx::query_scalar("SELECT grove_time.transaction_now()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(before, base());
    set_time(&pool, base() + Duration::days(1)).await;
    let (transaction, wall): (DateTime<Utc>, DateTime<Utc>) =
        sqlx::query_as("SELECT grove_time.transaction_now(), grove_time.wall_now()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(transaction, base());
    assert_eq!(wall, base() + Duration::days(1));
    tx.rollback().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn password_setup_expires_at_the_exact_boundary(pool: PgPool) {
    install_clock(&pool).await;
    let phc = "$argon2id$v=19$m=19456,t=2,p=1$abcdefghijklmnop$1234567890123456789012345678901234567890123";
    let owner = db::passwords::initialize(&pool, Uuid::new_v4(), "owner", "Owner", phc)
        .await
        .unwrap();
    let generation = db::passwords::find(&pool, "owner")
        .await
        .unwrap()
        .unwrap()
        .generation;
    let session_hash = hash(30);
    db::sessions::create_password_session(&pool, Uuid::new_v4(), owner, generation, &session_hash)
        .await
        .unwrap()
        .unwrap();
    let reader = user(&pool, grove_management_policy::Role::Reader).await;
    let token = hash(31);
    let setup = db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session_hash,
        reader,
        "reader",
        &token,
    )
    .await
    .unwrap();
    assert_eq!(setup.expires_at, base() + Duration::hours(24));
    set_time(&pool, setup.expires_at - Duration::microseconds(1)).await;
    assert!(
        db::password_setup::inspect(&pool, &token)
            .await
            .unwrap()
            .is_some()
    );
    set_time(&pool, setup.expires_at).await;
    assert!(
        db::password_setup::inspect(&pool, &token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &token, phc)
            .await
            .unwrap()
            .is_none()
    );
    let unchanged: bool = sqlx::query_scalar(
        "SELECT password_hash IS NULL FROM management.password_credentials WHERE account_id=$1",
    )
    .bind(reader)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(unchanged);
}

#[sqlx::test(migrations = "./migrations")]
async fn usage_history_cutoff_tracks_the_injected_calendar_date(pool: PgPool) {
    install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    sqlx::query("INSERT INTO usage_snapshots(day,storage_id,client_id,active_bytes,active_files) VALUES('2025-12-31','s','c',10,1)")
        .execute(&pool).await.unwrap();
    assert_eq!(
        grove_db::usage::snapshot_history(&pool, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    set_time(&pool, base() + Duration::days(1)).await;
    assert!(
        grove_db::usage::snapshot_history(&pool, 1)
            .await
            .unwrap()
            .is_empty()
    );
}
