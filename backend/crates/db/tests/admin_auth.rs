#![allow(clippy::unwrap_used)]

use filegate_db::{
    PgPool,
    admin_auth::{self as db, IssueMode},
};

#[sqlx::test(migrations = "./migrations")]
async fn initialization_is_singleton_and_recovery_invalidates_everything(pool: PgPool) {
    let (a, b) = tokio::join!(
        db::issue(&pool, IssueMode::Initialize, "a", "hash-a"),
        db::issue(&pool, IssueMode::Initialize, "b", "hash-b")
    );
    assert_ne!(a.unwrap().is_some(), b.unwrap().is_some());
    let created = db::issue(&pool, IssueMode::Create, "cli", "cli-hash")
        .await
        .unwrap()
        .unwrap();
    db::create_session(&pool, "cli-hash", "session-hash")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        db::session_actor(&pool, "session-hash").await.unwrap(),
        Some(created.id)
    );
    let recovered = db::issue(&pool, IssueMode::Recover, "recovery", "new-hash")
        .await
        .unwrap()
        .unwrap();
    assert!(db::authenticate(&pool, "cli-hash").await.unwrap().is_none());
    assert!(
        db::session_actor(&pool, "session-hash")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db::authenticate(&pool, "new-hash").await.unwrap(),
        Some(recovered.id)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_expire_are_capped_and_follow_credential_expiry(pool: PgPool) {
    let credential = db::issue(&pool, IssueMode::Initialize, "admin", "hash")
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE admin_credentials SET expires_at = now() + interval '1 hour'")
        .execute(&pool)
        .await
        .unwrap();
    for i in 0..65 {
        let (_, expires) = db::create_session(&pool, "hash", &format!("session-{i}"))
            .await
            .unwrap()
            .unwrap();
        assert!(expires < chrono::Utc::now() + chrono::Duration::hours(2));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 64);
    assert!(
        db::session_actor(&pool, "session-0")
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("UPDATE admin_sessions SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        db::session_actor(&pool, "session-64")
            .await
            .unwrap()
            .is_none()
    );
    db::create_session(&pool, "hash", "fresh")
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE admin_credentials SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(db::authenticate(&pool, "hash").await.unwrap().is_none());
    assert!(db::session_actor(&pool, "fresh").await.unwrap().is_none());
    assert!(
        db::create_session(&pool, "hash", "expired")
            .await
            .unwrap()
            .is_none()
    );
    assert!(db::revoke(&pool, credential.id).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn revocation_racing_login_leaves_no_usable_session(pool: PgPool) {
    let credential = db::issue(&pool, IssueMode::Initialize, "admin", "hash")
        .await
        .unwrap()
        .unwrap();
    let (revoke, login) = tokio::join!(
        db::revoke(&pool, credential.id),
        db::create_session(&pool, "hash", "session")
    );
    assert!(revoke.unwrap());
    login.unwrap();
    assert!(db::session_actor(&pool, "session").await.unwrap().is_none());
    assert!(
        db::create_session(&pool, "hash", "another")
            .await
            .unwrap()
            .is_none()
    );
}
