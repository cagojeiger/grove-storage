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
async fn issuance_racing_deletion_never_leaves_a_valid_key(pool: PgPool) {
    bootstrap(&pool).await;
    let account = user(&pool, Role::Writer).await;
    let ctx = context();
    let hash = hash(2);
    let key = key(&hash);
    let (issued, deleted) = tokio::join!(
        db::issue_credential(&pool, &ctx, account, &key),
        db::change_account(&pool, &ctx, account, AccountChange::Delete)
    );
    deleted.unwrap();
    assert!(issued.is_ok() || matches!(issued, Err(Error::InactiveAccount)));
    assert!(db::authenticate(&pool, &hash).await.unwrap().is_none());
    let valid: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.credentials WHERE account_id=$1 AND revoked_at IS NULL",
    )
    .bind(account)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(valid, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_failure_rolls_back_issuance_and_revocation(pool: PgPool) {
    let (owner, original) = bootstrap(&pool).await;
    db::create_session(&pool, uuid::Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap()
        .unwrap();
    let before = audit_count(&pool).await;
    reject_audit(&pool).await;
    assert!(
        db::issue_credential(&pool, &context(), owner, &key(&hash(2)))
            .await
            .is_err()
    );
    assert!(
        db::revoke_credential(&pool, &context(), original.id)
            .await
            .is_err()
    );
    assert!(db::authenticate(&pool, &hash(1)).await.unwrap().is_some());
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
    assert_eq!(audit_count(&pool).await, before);
}

#[sqlx::test(migrations = "./migrations")]
async fn hashes_are_unique_expiry_is_checked_and_audits_exclude_secrets(pool: PgPool) {
    let (owner, credential) = bootstrap(&pool).await;
    assert!(
        db::issue_credential(&pool, &context(), owner, &key(&hash(1)))
            .await
            .is_err()
    );
    let expired_hash = hash(2);
    let mut expired = key(&expired_hash);
    expired.expires_at = chrono::Utc::now() - chrono::Duration::seconds(1);
    assert!(
        db::issue_credential(&pool, &context(), owner, &expired)
            .await
            .is_err()
    );
    let audit: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(e)::text FROM management.audit_events e")
            .fetch_all(&pool)
            .await
            .unwrap();
    for event in audit {
        assert!(!event.contains(&hash(1)));
        assert!(!event.contains("gst_test"));
    }
    assert!(
        db::revoke_credential(&pool, &context(), credential.id)
            .await
            .unwrap()
    );
    let before = audit_count(&pool).await;
    assert!(
        !db::revoke_credential(&pool, &context(), credential.id)
            .await
            .unwrap()
    );
    assert_eq!(audit_count(&pool).await, before);
}
