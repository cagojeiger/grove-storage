#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{
    PgPool,
    management::{self as db, AccountChange},
};
use grove_management_policy::Role;
use support::*;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn revocation_and_login_are_serialized(pool: PgPool) {
    let (_, credential) = bootstrap(&pool).await;
    let ctx = context();
    let token = hash(1);
    let session = hash(10);
    let (revoke, login) = tokio::join!(
        db::revoke_credential(&pool, &ctx, credential.id),
        db::create_session(&pool, Uuid::new_v4(), &token, &session)
    );
    assert!(revoke.unwrap());
    login.unwrap();
    assert!(db::session_actor(&pool, &session).await.unwrap().is_none());
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &token, &hash(11))
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn tokens_can_login_and_disabled_users_cannot_resume_old_sessions(pool: PgPool) {
    bootstrap(&pool).await;
    let owner = user(&pool, Role::Operator).await;
    db::issue_credential(&pool, &context(), owner, &key(&hash(2)))
        .await
        .unwrap();
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(2), &hash(10))
            .await
            .unwrap()
            .is_some()
    );
    db::issue_credential(&pool, &context(), owner, &key(&hash(3)))
        .await
        .unwrap();
    db::create_session(&pool, Uuid::new_v4(), &hash(3), &hash(11))
        .await
        .unwrap()
        .unwrap();
    db::change_account(&pool, &context(), owner, AccountChange::Active(false))
        .await
        .unwrap();
    assert!(db::authenticate(&pool, &hash(3)).await.unwrap().is_none());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
    db::change_account(&pool, &context(), owner, AccountChange::Active(true))
        .await
        .unwrap();
    assert!(db::authenticate(&pool, &hash(3)).await.unwrap().is_some());
    assert!(db::session_actor(&pool, &hash(11)).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn session_bounds_and_parent_expiry_are_enforced(pool: PgPool) {
    let (_, key) = bootstrap(&pool).await;
    for n in 100..165 {
        let session = db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(n))
            .await
            .unwrap()
            .unwrap();
        assert!(session.expires_at <= key.expires_at);
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.sessions WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 64);
    assert!(
        db::session_actor(&pool, &hash(100))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db::session_actor(&pool, &hash(164))
            .await
            .unwrap()
            .is_some()
    );
    reject_audit(&pool).await;
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(165))
            .await
            .is_err()
    );
    assert!(
        db::session_actor(&pool, &hash(101))
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        db::session_actor(&pool, &hash(165))
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("UPDATE management.credentials SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 hour'").execute(&pool).await.unwrap();
    assert!(
        db::session_actor(&pool, &hash(164))
            .await
            .unwrap()
            .is_none()
    );
    assert!(db::authenticate(&pool, &hash(1)).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn long_lived_tokens_still_get_fixed_eight_hour_sessions(pool: PgPool) {
    let (owner, _) = bootstrap(&pool).await;
    let token_hash = hash(2);
    let mut credential = key(&token_hash);
    credential.expires_at = chrono::Utc::now() + chrono::Duration::days(90);
    db::issue_credential(&pool, &context(), owner, &credential)
        .await
        .unwrap();
    let session = db::create_session(&pool, Uuid::new_v4(), &token_hash, &hash(10))
        .await
        .unwrap()
        .unwrap();
    let bounded: bool = sqlx::query_scalar("SELECT expires_at <= created_at+interval '8 hours' AND expires_at > created_at+interval '7 hours' FROM management.sessions WHERE id=$1")
        .bind(session.id).fetch_one(&pool).await.unwrap();
    assert!(bounded);
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_are_owner_scoped_and_audit_failure_rolls_back(pool: PgPool) {
    let (owner, _) = bootstrap(&pool).await;
    let other = user(&pool, Role::Viewer).await;
    let session = db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap()
        .unwrap();
    assert!(
        !db::revoke_session(&pool, &context(), other, session.id)
            .await
            .unwrap()
    );
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    reject_audit(&pool).await;
    assert!(
        db::revoke_session(&pool, &context(), owner, session.id)
            .await
            .is_err()
    );
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(11))
            .await
            .is_err()
    );
    assert!(db::session_actor(&pool, &hash(11)).await.unwrap().is_none());
}
