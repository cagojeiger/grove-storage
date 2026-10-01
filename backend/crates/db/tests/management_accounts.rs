#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{
    PgPool,
    management::{self as db, AccountChange, Error},
};
use grove_management_policy::{Action, Role, Surface, authorize};
use support::*;

#[sqlx::test(migrations = "./migrations")]
async fn bootstrap_is_atomic_and_singleton(pool: PgPool) {
    let hash_a = hash(1);
    let hash_b = hash(2);
    let a = key(&hash_a);
    let b = key(&hash_b);
    let ctx = context();
    let (a, b) = tokio::join!(
        db::bootstrap(&pool, &ctx, "A", &a),
        db::bootstrap(&pool, &ctx, "B", &b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(
        matches!(a, Err(Error::AlreadyInitialized)) || matches!(b, Err(Error::AlreadyInitialized))
    );
    assert_eq!(audit_count(&pool).await, 2);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn last_admin_changes_are_serialized(pool: PgPool) {
    let (first, _) = bootstrap(&pool).await;
    let second = user(&pool, Role::Admin).await;
    let ctx = context();
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *fence)
        .await
        .unwrap();
    let a_pool = pool.clone();
    let a = tokio::spawn(async move {
        db::change_account(&a_pool, &ctx, first, AccountChange::Role(Role::Reader)).await
    });
    let b_pool = pool.clone();
    let b = tokio::spawn(async move {
        db::change_account(&b_pool, &ctx, second, AccountChange::Active(false)).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory'")
                .fetch_one(&pool).await.unwrap();
            if waiting == 2 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    fence.commit().await.unwrap();
    let (a, b) = (a.await.unwrap(), b.await.unwrap());
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(matches!(a, Err(Error::LastAdmin)) || matches!(b, Err(Error::LastAdmin)));
    let remaining: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM management.accounts WHERE role='admin' AND is_active")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(matches!(
        db::change_account(&pool, &ctx, remaining, AccountChange::Delete).await,
        Err(Error::LastAdmin)
    ));
    assert_eq!(audit_count(&pool).await, 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn token_authorization_tracks_current_user_and_deletion(pool: PgPool) {
    bootstrap(&pool).await;
    let owner = user(&pool, Role::Writer).await;
    db::issue_credential(&pool, &context(), owner, &key(&hash(2)))
        .await
        .unwrap();
    let actor = db::authenticate(&pool, &hash(2)).await.unwrap().unwrap();
    assert_eq!(actor.account_id, owner);
    assert!(authorize(actor.caller, Surface::Mcp, Action::WriteResources).is_ok());
    db::change_account(&pool, &context(), owner, AccountChange::Role(Role::Reader))
        .await
        .unwrap();
    let actor = db::authenticate(&pool, &hash(2)).await.unwrap().unwrap();
    assert!(authorize(actor.caller, Surface::Mcp, Action::WriteResources).is_err());
    assert!(authorize(actor.caller, Surface::Cli, Action::ReadResources).is_ok());
    db::change_account(&pool, &context(), owner, AccountChange::Delete)
        .await
        .unwrap();
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
    assert!(matches!(
        db::issue_credential(&pool, &context(), owner, &key(&hash(3))).await,
        Err(Error::InactiveAccount)
    ));
    let retained: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.audit_events WHERE resource_id=$1")
            .bind(owner.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retained, 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn no_op_has_no_duplicate_audit_and_failures_roll_back(pool: PgPool) {
    bootstrap(&pool).await;
    let account = user(&pool, Role::Reader).await;
    let before = audit_count(&pool).await;
    assert!(
        !db::change_account(
            &pool,
            &context(),
            account,
            AccountChange::Role(Role::Reader)
        )
        .await
        .unwrap()
    );
    assert_eq!(audit_count(&pool).await, before);
    reject_audit(&pool).await;
    assert!(
        db::change_account(&pool, &context(), account, AccountChange::Role(Role::Admin))
            .await
            .is_err()
    );
    let role: String = sqlx::query_scalar("SELECT role FROM management.accounts WHERE id=$1")
        .bind(account)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role, "reader");
    assert!(
        db::create_account(
            &pool,
            &context(),
            db::NewAccount {
                display_name: "lost",
                role: Role::Reader
            }
        )
        .await
        .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_bootstrap_leaves_no_account_or_credential(pool: PgPool) {
    reject_audit(&pool).await;
    assert!(
        db::bootstrap(&pool, &context(), "Owner", &key(&hash(1)))
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM management.accounts)+(SELECT count(*) FROM management.credentials)").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_delete_preserves_all_user_tokens_and_session(pool: PgPool) {
    bootstrap(&pool).await;
    let owner = user(&pool, Role::Writer).await;
    db::issue_credential(&pool, &context(), owner, &key(&hash(2)))
        .await
        .unwrap();
    db::issue_credential(&pool, &context(), owner, &key(&hash(3)))
        .await
        .unwrap();
    db::create_session(&pool, uuid::Uuid::new_v4(), &hash(2), &hash(10))
        .await
        .unwrap()
        .unwrap();
    let before = audit_count(&pool).await;
    reject_audit(&pool).await;
    assert!(
        db::change_account(&pool, &context(), owner, AccountChange::Delete)
            .await
            .is_err()
    );
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_some());
    assert!(db::authenticate(&pool, &hash(3)).await.unwrap().is_some());
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    assert_eq!(audit_count(&pool).await, before);
}
