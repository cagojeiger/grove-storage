#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use grove_db::{
    PgPool,
    management::{self as db, Error, passwords},
};
use grove_management_policy::Role;
use support::*;
use uuid::Uuid;

// Persistence tests use PHC-shaped fixtures; cryptographic verification is tested in the service.
const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXJhbmRvbXNhbHQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const NEXT_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXJhbmRvbXNhbHQ$BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

#[sqlx::test(migrations = "./migrations")]
async fn initialization_requires_an_active_password_admin(pool: PgPool) {
    assert!(!passwords::initialized(&pool).await.unwrap());
    let (owner, _) = bootstrap(&pool).await;
    assert!(!passwords::initialized(&pool).await.unwrap());
    passwords::recover(&pool, Uuid::new_v4(), owner, "owner", HASH)
        .await
        .unwrap();
    assert!(passwords::initialized(&pool).await.unwrap());
    sqlx::query("UPDATE management.accounts SET is_active=false WHERE id=$1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!passwords::initialized(&pool).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_initialization_has_one_account_and_audit(pool: PgPool) {
    let (a, b) = tokio::join!(
        passwords::initialize(&pool, Uuid::new_v4(), "owner-a", "A", HASH),
        passwords::initialize(&pool, Uuid::new_v4(), "owner-b", "B", HASH)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(
        matches!(a, Err(Error::AlreadyInitialized)) || matches!(b, Err(Error::AlreadyInitialized))
    );
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM management.accounts),
                (SELECT count(*) FROM management.password_credentials),
                (SELECT count(*) FROM management.api_tokens)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1, 0));
    let event: (String, String, String, Option<Uuid>, serde_json::Value) = sqlx::query_as(
        "SELECT actor_kind,surface,action,actor_id,metadata FROM management.audit_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        event,
        (
            "system".into(),
            "local".into(),
            "account.initialize".into(),
            None,
            serde_json::json!({})
        )
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_failure_rolls_back_initialization(pool: PgPool) {
    reject_audit(&pool).await;
    assert!(
        passwords::initialize(&pool, Uuid::new_v4(), "owner", "Owner", HASH)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM management.accounts)+(SELECT count(*) FROM management.password_credentials)",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn recovery_replaces_generation_and_preserves_other_identities(pool: PgPool) {
    let (owner, _) = bootstrap(&pool).await;
    let other = user(&pool, Role::Writer).await;
    db::issue_credential(&pool, &context(), other, &key(&hash(2)))
        .await
        .unwrap();
    db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap();
    db::create_session(&pool, Uuid::new_v4(), &hash(2), &hash(20))
        .await
        .unwrap();
    passwords::recover(&pool, Uuid::new_v4(), owner, "owner", HASH)
        .await
        .unwrap();
    let first = passwords::find(&pool, "owner").await.unwrap().unwrap();
    passwords::recover(&pool, Uuid::new_v4(), owner, "owner", NEXT_HASH)
        .await
        .unwrap();
    let second = passwords::find(&pool, "owner").await.unwrap().unwrap();
    assert_ne!(first.generation, second.generation);
    assert_eq!(second.password_hash, NEXT_HASH);
    assert_eq!(second.account_id, owner);
    assert!(db::authenticate(&pool, &hash(1)).await.unwrap().is_none());
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_none());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_some());
    assert!(db::session_actor(&pool, &hash(20)).await.unwrap().is_some());
    let role: String = sqlx::query_scalar("SELECT role FROM management.accounts WHERE id=$1")
        .bind(owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role, "admin");
}

#[sqlx::test(migrations = "./migrations")]
async fn recovery_audit_failure_preserves_hash_token_and_session(pool: PgPool) {
    let (owner, _) = bootstrap(&pool).await;
    passwords::recover(&pool, Uuid::new_v4(), owner, "owner", HASH)
        .await
        .unwrap();
    db::issue_credential(&pool, &context(), owner, &key(&hash(2)))
        .await
        .unwrap();
    db::create_session(&pool, Uuid::new_v4(), &hash(2), &hash(20))
        .await
        .unwrap();
    let before = passwords::find(&pool, "owner").await.unwrap().unwrap();
    let before_audit = audit_count(&pool).await;
    reject_audit(&pool).await;
    assert!(
        passwords::recover(&pool, Uuid::new_v4(), owner, "owner", NEXT_HASH)
            .await
            .is_err()
    );
    let after = passwords::find(&pool, "owner").await.unwrap().unwrap();
    assert_eq!(before.generation, after.generation);
    assert_eq!(after.password_hash, HASH);
    assert!(passwords::find(&pool, "renamed").await.unwrap().is_none());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_some());
    assert!(db::session_actor(&pool, &hash(20)).await.unwrap().is_some());
    assert_eq!(audit_count(&pool).await, before_audit);
}

#[sqlx::test(migrations = "./migrations")]
async fn usernames_are_unique_canonical_and_retained_after_account_deletion(pool: PgPool) {
    passwords::initialize(&pool, Uuid::new_v4(), "owner", "Owner", HASH)
        .await
        .unwrap();
    let second = user(&pool, Role::Reader).await;
    assert!(
        passwords::recover(&pool, Uuid::new_v4(), second, "owner", HASH)
            .await
            .is_err()
    );
    for invalid in ["Aaa", "a", "a b", "_name", "name@host"] {
        assert!(
            passwords::recover(&pool, Uuid::new_v4(), second, invalid, HASH)
                .await
                .is_err()
        );
    }
    passwords::recover(&pool, Uuid::new_v4(), second, "second", HASH)
        .await
        .unwrap();
    db::change_account(&pool, &context(), second, db::AccountChange::Delete)
        .await
        .unwrap();
    assert!(passwords::find(&pool, "second").await.unwrap().is_none());
    let third = user(&pool, Role::Reader).await;
    assert!(
        passwords::recover(&pool, Uuid::new_v4(), third, "second", HASH)
            .await
            .is_err()
    );
    assert!(passwords::find(&pool, "owner").await.unwrap().is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn recovery_preserves_the_existing_login_name(pool: PgPool) {
    let account = passwords::initialize(&pool, Uuid::new_v4(), "owner", "Owner", HASH)
        .await
        .unwrap();
    let before = passwords::find(&pool, "owner").await.unwrap().unwrap();
    assert!(matches!(
        passwords::recover(&pool, Uuid::new_v4(), account, "renamed", NEXT_HASH).await,
        Err(Error::InvalidInput)
    ));
    let after = passwords::find(&pool, "owner").await.unwrap().unwrap();
    assert_eq!(before.generation, after.generation);
    assert_eq!(after.password_hash, HASH);
    assert!(passwords::find(&pool, "renamed").await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn disabled_accounts_cannot_be_recovered_or_authenticated(pool: PgPool) {
    passwords::initialize(&pool, Uuid::new_v4(), "owner", "Owner", HASH)
        .await
        .unwrap();
    let account = user(&pool, Role::Writer).await;
    passwords::recover(&pool, Uuid::new_v4(), account, "writer", HASH)
        .await
        .unwrap();
    db::change_account(&pool, &context(), account, db::AccountChange::Active(false))
        .await
        .unwrap();
    assert!(passwords::find(&pool, "writer").await.unwrap().is_none());
    assert!(matches!(
        passwords::recover(&pool, Uuid::new_v4(), account, "writer", NEXT_HASH).await,
        Err(Error::InactiveAccount)
    ));
    assert!(matches!(
        passwords::recover(&pool, Uuid::new_v4(), Uuid::new_v4(), "missing", HASH).await,
        Err(Error::InactiveAccount)
    ));
}
