#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{
    PgPool,
    management::{self as db, Error},
};
use support::*;

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_issuance_respects_limit_and_releases_revoked_or_expired_slots(pool: PgPool) {
    let (owner, original) = bootstrap(&pool).await;
    for value in 2..=31 {
        db::issue_credential(&pool, &context(), owner, &key(&hash(value)))
            .await
            .unwrap();
    }
    let ctx = context();
    let first_hash = hash(32);
    let second_hash = hash(33);
    let first_key = key(&first_hash);
    let second_key = key(&second_hash);
    let before = audit_count(&pool).await;
    let (first, second) = tokio::join!(
        db::issue_credential(&pool, &ctx, owner, &first_key),
        db::issue_credential(&pool, &ctx, owner, &second_key)
    );
    assert!(matches!(
        (&first, &second),
        (Ok(_), Err(Error::CredentialLimit)) | (Err(Error::CredentialLimit), Ok(_))
    ));
    assert_eq!(audit_count(&pool).await, before + 1);
    assert_eq!(active_count(&pool, owner).await, 32);

    db::revoke_credential(&pool, &context(), original.id)
        .await
        .unwrap();
    let replacement = db::issue_credential(&pool, &context(), owner, &key(&hash(34)))
        .await
        .unwrap();
    assert_eq!(active_count(&pool, owner).await, 32);
    sqlx::query("UPDATE management.credentials SET created_at=clock_timestamp()-interval '2 days', expires_at=clock_timestamp()-interval '1 day' WHERE id=$1")
        .bind(replacement.id).execute(&pool).await.unwrap();
    db::issue_credential(&pool, &context(), owner, &key(&hash(35)))
        .await
        .unwrap();
    assert_eq!(active_count(&pool, owner).await, 32);

    const PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXJhbmRvbXNhbHQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    db::passwords::recover(&pool, uuid::Uuid::new_v4(), owner, "owner", PHC)
        .await
        .unwrap();
    assert_eq!(active_count(&pool, owner).await, 0);
    db::issue_credential(&pool, &context(), owner, &key(&hash(36)))
        .await
        .unwrap();
    assert_eq!(active_count(&pool, owner).await, 1);
}

async fn active_count(pool: &PgPool, account: uuid::Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM management.credentials WHERE account_id=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp()")
        .bind(account).fetch_one(pool).await.unwrap()
}
