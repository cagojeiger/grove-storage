#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{PgPool, management as db};
use grove_management_policy::Role;
use support::*;
use uuid::Uuid;

const PHC: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$abcdefghijklmnop$1234567890123456789012345678901234567890123";

async fn owner(pool: &PgPool) -> (Uuid, String) {
    let id = db::passwords::initialize(pool, Uuid::new_v4(), "owner", "Owner", PHC)
        .await
        .unwrap();
    let generation = db::passwords::find(pool, "owner")
        .await
        .unwrap()
        .unwrap()
        .generation;
    let session_hash = hash(10);
    db::sessions::create_password_session(pool, Uuid::new_v4(), id, generation, &session_hash)
        .await
        .unwrap()
        .unwrap();
    (id, session_hash)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn setup_reserves_name_reissue_invalidates_old_token_and_completion_is_single_use(
    pool: PgPool,
) {
    let (_, session) = owner(&pool).await;
    let account = user(&pool, Role::Writer).await;
    let first = db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        account,
        "writer.name",
        &hash(2),
    )
    .await
    .unwrap();
    assert_eq!(first.account_id, account);
    assert_eq!(first.login_name, "writer.name");
    assert!(first.expires_at > chrono::Utc::now());
    assert!(
        db::passwords::find(&pool, "writer.name")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db::password_setup::inspect(&pool, &hash(2))
            .await
            .unwrap()
            .is_some()
    );
    let duplicate = user(&pool, Role::Reader).await;
    assert!(
        db::password_setup::issue(
            &pool,
            Uuid::new_v4(),
            &session,
            duplicate,
            "writer.name",
            &hash(9),
        )
        .await
        .is_err()
    );
    db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        account,
        "writer.name",
        &hash(3),
    )
    .await
    .unwrap();
    assert!(
        db::password_setup::inspect(&pool, &hash(2))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(2), PHC)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(3), PHC)
            .await
            .unwrap(),
        Some(account)
    );
    assert_eq!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(3), PHC)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        db::passwords::find(&pool, "writer.name")
            .await
            .unwrap()
            .unwrap()
            .account_id,
        account
    );
    assert!(
        db::password_setup::issue(
            &pool,
            Uuid::new_v4(),
            &session,
            account,
            "writer.name",
            &hash(4),
        )
        .await
        .is_err()
    );
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM management.audit_events WHERE resource_id=$1 AND action LIKE 'account.password_setup.%' ORDER BY id",
    ).bind(account.to_string()).fetch_all(&pool).await.unwrap();
    assert_eq!(
        actions,
        [
            "account.password_setup.issue",
            "account.password_setup.issue",
            "account.password_setup.complete"
        ]
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn expired_deleted_or_recovered_setup_cannot_complete(pool: PgPool) {
    let (_, session) = owner(&pool).await;
    let expired = user(&pool, Role::Reader).await;
    db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        expired,
        "expired",
        &hash(2),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE management.password_setup_tokens SET created_at=clock_timestamp()-interval '2 days',expires_at=clock_timestamp()-interval '1 second' WHERE account_id=$1")
        .bind(expired).execute(&pool).await.unwrap();
    assert!(
        db::password_setup::inspect(&pool, &hash(2))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(2), PHC)
            .await
            .unwrap(),
        None
    );
    let deleted = user(&pool, Role::Reader).await;
    db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        deleted,
        "deleted",
        &hash(3),
    )
    .await
    .unwrap();
    db::change_account(&pool, &context(), deleted, db::AccountChange::Delete)
        .await
        .unwrap();
    assert!(
        db::password_setup::inspect(&pool, &hash(3))
            .await
            .unwrap()
            .is_none()
    );
    let recovered = user(&pool, Role::Reader).await;
    db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        recovered,
        "recovered",
        &hash(4),
    )
    .await
    .unwrap();
    db::passwords::recover(&pool, Uuid::new_v4(), recovered, "recovered", PHC)
        .await
        .unwrap();
    assert!(
        db::password_setup::inspect(&pool, &hash(4))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(4), PHC)
            .await
            .unwrap(),
        None
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn completion_has_one_winner_and_audit_failure_rolls_back(pool: PgPool) {
    let (_, session) = owner(&pool).await;
    let account = user(&pool, Role::Reader).await;
    db::password_setup::issue(
        &pool,
        Uuid::new_v4(),
        &session,
        account,
        "concurrent",
        &hash(2),
    )
    .await
    .unwrap();
    let challenge = hash(2);
    let (left, right) = tokio::join!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &challenge, PHC),
        db::password_setup::complete(&pool, Uuid::new_v4(), &challenge, PHC),
    );
    assert_eq!(
        [left.unwrap(), right.unwrap()]
            .iter()
            .filter(|result| **result == Some(account))
            .count(),
        1
    );
    let other = user(&pool, Role::Reader).await;
    db::password_setup::issue(&pool, Uuid::new_v4(), &session, other, "rollback", &hash(3))
        .await
        .unwrap();
    reject_audit(&pool).await;
    assert!(
        db::password_setup::complete(&pool, Uuid::new_v4(), &hash(3), PHC)
            .await
            .is_err()
    );
    assert!(
        db::password_setup::inspect(&pool, &hash(3))
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        db::passwords::find(&pool, "rollback")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn only_current_admin_password_sessions_issue_setup(pool: PgPool) {
    let (admin, session) = owner(&pool).await;
    let target = user(&pool, Role::Reader).await;
    assert!(
        db::password_setup::issue(&pool, Uuid::new_v4(), &hash(99), target, "target", &hash(2))
            .await
            .is_err()
    );
    let other_admin = user(&pool, Role::Admin).await;
    db::passwords::recover(&pool, Uuid::new_v4(), other_admin, "other-admin", PHC)
        .await
        .unwrap();
    db::change_account(
        &pool,
        &context(),
        admin,
        db::AccountChange::Role(Role::Writer),
    )
    .await
    .unwrap();
    assert!(
        db::password_setup::issue(&pool, Uuid::new_v4(), &session, target, "target", &hash(2))
            .await
            .is_err()
    );
    assert!(other_admin != admin);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn create_with_setup_is_atomic_and_requires_current_admin(pool: PgPool) {
    let (admin, session) = owner(&pool).await;
    let first = db::password_setup::create(
        &pool,
        Uuid::new_v4(),
        &session,
        db::NewAccount {
            display_name: "New writer",
            role: Role::Writer,
        },
        "new.writer",
        &hash(2),
    )
    .await
    .unwrap();
    assert_eq!(first.login_name, "new.writer");
    assert!(
        db::password_setup::inspect(&pool, &hash(2))
            .await
            .unwrap()
            .is_some()
    );
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM management.audit_events WHERE resource_id=$1 ORDER BY id",
    )
    .bind(first.account_id.to_string())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions, ["account.create", "account.password_setup.issue"]);

    let duplicate = db::password_setup::create(
        &pool,
        Uuid::new_v4(),
        &session,
        db::NewAccount {
            display_name: "Duplicate",
            role: Role::Reader,
        },
        "new.writer",
        &hash(3),
    )
    .await;
    assert!(duplicate.is_err());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.accounts WHERE display_name='Duplicate'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);

    reject_audit(&pool).await;
    assert!(
        db::password_setup::create(
            &pool,
            Uuid::new_v4(),
            &session,
            db::NewAccount {
                display_name: "Rolled back",
                role: Role::Reader
            },
            "rolled.back",
            &hash(4),
        )
        .await
        .is_err()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.accounts WHERE display_name='Rolled back'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_audit ON management.audit_events")
        .execute(&pool)
        .await
        .unwrap();

    let extra_admin = user(&pool, Role::Admin).await;
    db::passwords::recover(&pool, Uuid::new_v4(), extra_admin, "extra-admin", PHC)
        .await
        .unwrap();
    db::change_account(
        &pool,
        &context(),
        admin,
        db::AccountChange::Role(Role::Writer),
    )
    .await
    .unwrap();
    assert_ne!(extra_admin, admin);
    assert!(matches!(
        db::password_setup::create(
            &pool,
            Uuid::new_v4(),
            &session,
            db::NewAccount {
                display_name: "Forbidden",
                role: Role::Reader
            },
            "forbidden",
            &hash(5),
        )
        .await,
        Err(db::Error::Forbidden)
    ));
}
