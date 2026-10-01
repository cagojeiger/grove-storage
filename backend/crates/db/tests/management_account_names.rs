#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{
    PgPool,
    management::{self as db, AccountChange, Error},
};
use grove_management_policy::Role;
use support::*;

#[sqlx::test(migrations = "./migrations")]
async fn rename_preserves_identity_credentials_and_sessions(pool: PgPool) {
    let (id, _) = bootstrap(&pool).await;
    db::create_session(&pool, uuid::Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap()
        .unwrap();
    let before = audit_count(&pool).await;
    assert!(
        db::change_account(
            &pool,
            &context(),
            id,
            AccountChange::Name("  New owner  ".into())
        )
        .await
        .unwrap()
    );
    assert!(
        !db::change_account(
            &pool,
            &context(),
            id,
            AccountChange::Name("New owner".into())
        )
        .await
        .unwrap()
    );
    let row: (String, String, bool) =
        sqlx::query_as("SELECT display_name,role,is_active FROM management.accounts WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("New owner".into(), "admin".into(), true));
    assert_eq!(
        db::authenticate(&pool, &hash(1))
            .await
            .unwrap()
            .unwrap()
            .account_id,
        id
    );
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    assert_eq!(audit_count(&pool).await, before + 1);
    let metadata: serde_json::Value = sqlx::query_scalar(
        "SELECT metadata FROM management.audit_events WHERE action='account.name'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        metadata,
        serde_json::json!({"before_name":"Owner","after_name":"New owner"})
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_names_and_audit_failure_leave_name_unchanged(pool: PgPool) {
    let (id, _) = bootstrap(&pool).await;
    for name in ["  ".to_string(), "x".repeat(81)] {
        assert!(matches!(
            db::change_account(&pool, &context(), id, AccountChange::Name(name)).await,
            Err(Error::InvalidInput)
        ));
    }
    let unicode = "\u{AC00}".repeat(80);
    assert!(
        db::change_account(&pool, &context(), id, AccountChange::Name(unicode.clone()))
            .await
            .unwrap()
    );
    reject_audit(&pool).await;
    assert!(
        db::change_account(&pool, &context(), id, AccountChange::Name("Lost".into()))
            .await
            .is_err()
    );
    let name: String =
        sqlx::query_scalar("SELECT display_name FROM management.accounts WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, unicode);
}

#[sqlx::test(migrations = "./migrations")]
async fn disabled_can_be_renamed_but_deleted_and_missing_cannot(pool: PgPool) {
    bootstrap(&pool).await;
    let id = user(&pool, Role::Reader).await;
    db::change_account(&pool, &context(), id, AccountChange::Active(false))
        .await
        .unwrap();
    db::change_account(
        &pool,
        &context(),
        id,
        AccountChange::Name("Disabled".into()),
    )
    .await
    .unwrap();
    let active: bool = sqlx::query_scalar("SELECT is_active FROM management.accounts WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!active);
    db::change_account(&pool, &context(), id, AccountChange::Delete)
        .await
        .unwrap();
    for target in [id, uuid::Uuid::new_v4()] {
        assert!(matches!(
            db::change_account(
                &pool,
                &context(),
                target,
                AccountChange::Name("Lost".into())
            )
            .await,
            Err(Error::NotFound)
        ));
    }
}
