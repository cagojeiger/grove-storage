#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use grove_db::{PgPool, management as db};
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

fn key<'a>(label: &'a str, suffix: &'a str) -> db::NewCredential<'a> {
    db::NewCredential {
        label,
        token_prefix: "gsm_test",
        token_hash: suffix,
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn own_tokens_are_scoped_to_password_session_and_current_account(pool: PgPool) {
    let (owner, session) = owner(&pool).await;
    let other = user(&pool, Role::Reader).await;
    let outsider = db::issue_credential(&pool, &context(), other, &key("other", &hash(2)))
        .await
        .unwrap();
    let issued =
        db::personal_tokens::issue(&pool, Uuid::new_v4(), &session, &key("own CLI", &hash(3)))
            .await
            .unwrap();
    assert_eq!(issued.account_id, owner);
    let page = db::personal_tokens::list(&pool, &session, db::queries::Page::default())
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page.first().map(|row| row.id), Some(issued.id));
    assert!(matches!(
        db::personal_tokens::revoke(&pool, Uuid::new_v4(), &session, outsider.id).await,
        Err(db::Error::NotFound)
    ));
    assert!(
        db::personal_tokens::revoke(&pool, Uuid::new_v4(), &session, issued.id)
            .await
            .unwrap()
    );
    assert!(
        !db::personal_tokens::revoke(&pool, Uuid::new_v4(), &session, issued.id)
            .await
            .unwrap()
    );
    assert!(db::authenticate(&pool, &hash(3)).await.unwrap().is_none());
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM management.audit_events WHERE resource_type='credential' AND resource_id=$1 ORDER BY id",
    )
    .bind(issued.id.to_string())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions, ["credential.issue", "credential.revoke"]);

    db::change_account(&pool, &context(), owner, db::AccountChange::Active(false))
        .await
        .unwrap_err();
    assert!(
        db::personal_tokens::issue(&pool, Uuid::new_v4(), &hash(99), &key("bad", &hash(4)))
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn token_login_sessions_cannot_manage_personal_tokens(pool: PgPool) {
    let (owner, _) = owner(&pool).await;
    let issued = db::issue_credential(&pool, &context(), owner, &key("legacy", &hash(5)))
        .await
        .unwrap();
    let legacy_session = hash(6);
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(5), &legacy_session)
            .await
            .unwrap()
            .is_some_and(|session| session.credential_id == Some(issued.id))
    );
    assert!(matches!(
        db::personal_tokens::list(&pool, &legacy_session, db::queries::Page::default()).await,
        Err(db::Error::Forbidden)
    ));
    assert!(matches!(
        db::personal_tokens::issue(
            &pool,
            Uuid::new_v4(),
            &legacy_session,
            &key("new", &hash(7))
        )
        .await,
        Err(db::Error::Forbidden)
    ));
}
