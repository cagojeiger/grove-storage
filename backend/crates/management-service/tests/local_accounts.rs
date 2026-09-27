#![allow(clippy::unwrap_used)]

use filegate_core::{ExposeSecret, SecretString};
use filegate_db::{PgPool, management::passwords as db};
use grove_management_service::{Error, local_accounts, passwords};
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn local_lifecycle_uses_real_password_hashes_and_preserves_identity(pool: PgPool) {
    let password = SecretString::from("a private phrase for local provisioning");
    let account = local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        " Owner ",
        " Owner ",
        password.clone(),
    )
    .await
    .unwrap();
    let credential = db::find(&pool, "owner").await.unwrap().unwrap();
    assert_eq!(credential.account_id, account);
    assert!(
        passwords::verify(password.clone(), credential.password_hash.into())
            .await
            .unwrap()
    );
    assert!(matches!(
        local_accounts::initialize(
            &pool,
            Uuid::new_v4(),
            "another",
            "Another",
            password.clone()
        )
        .await,
        Err(Error::Conflict)
    ));
    let next = SecretString::from("a different phrase for local recovery");
    local_accounts::recover(&pool, Uuid::new_v4(), account, "owner", next.clone())
        .await
        .unwrap();
    let recovered = db::find(&pool, "owner").await.unwrap().unwrap();
    assert_eq!(account, recovered.account_id);
    assert_ne!(credential.generation, recovered.generation);
    assert!(
        !passwords::verify(password.clone(), recovered.password_hash.clone().into())
            .await
            .unwrap()
    );
    assert!(
        passwords::verify(next.clone(), recovered.password_hash.into())
            .await
            .unwrap()
    );
    let events: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(e) FROM management.audit_events e")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(events.len(), 2);
    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains(password.expose_secret()));
    assert!(!serialized.contains(next.expose_secret()));
    assert!(!serialized.contains("argon2"));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn invalid_inputs_never_create_an_identity(pool: PgPool) {
    for (login, name, password) in [
        ("owner", "Owner", "qwer1234"),
        ("a b", "Owner", "a private phrase for local provisioning"),
        ("owner", " ", "a private phrase for local provisioning"),
        ("owner", "Owner", "correct horse battery staple"),
    ] {
        assert!(matches!(
            local_accounts::initialize(&pool, Uuid::new_v4(), login, name, password.into()).await,
            Err(Error::InvalidInput)
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
