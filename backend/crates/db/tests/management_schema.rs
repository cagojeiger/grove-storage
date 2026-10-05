#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{PgPool, management as db};
use grove_management_policy::Role;
use support::*;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn accounts_are_complete_without_a_subtype_table(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO management.accounts(id,display_name,role) VALUES($1,'orphan','reader')",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let kind_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema='management' AND table_name='accounts' AND column_name='kind')")
        .fetch_one(&pool).await.unwrap();
    assert!(!kind_exists);
    let agents: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('management.agents')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(agents.is_none());
    let users: Option<String> = sqlx::query_scalar("SELECT to_regclass('management.users')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(users.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn session_fk_rejects_cross_account_credentials(pool: PgPool) {
    let (owner, credential) = bootstrap(&pool).await;
    let other = user(&pool, Role::Reader).await;
    let automation_user = owner;
    let token_key = db::issue_credential(&pool, &context(), automation_user, &key(&hash(2)))
        .await
        .unwrap();
    for (account_id, credential_id) in [(other, credential.id), (other, token_key.id)] {
        let error = sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,credential_id,expires_at) VALUES($1,$2,'token',$3,$4,clock_timestamp()+interval '1 hour')")
            .bind(Uuid::new_v4()).bind(hash(10)).bind(account_id).bind(credential_id).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23503")
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_require_one_account_and_exactly_one_authentication_method(pool: PgPool) {
    let (owner, credential) = bootstrap(&pool).await;
    for (method, token, generation) in [
        ("master", None, None),
        ("token", None, None),
        ("token", Some(credential.id), Some(Uuid::new_v4())),
        ("password", None, None),
        ("password", Some(credential.id), Some(Uuid::new_v4())),
    ] {
        let error = sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,credential_id,password_generation,expires_at) VALUES($1,$2,$3,$4,$5,$6,clock_timestamp()+interval '1 hour')")
            .bind(Uuid::new_v4()).bind(hash(10)).bind(method).bind(owner).bind(token).bind(generation)
            .execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let error = sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,credential_id,expires_at) VALUES($1,$2,'token',$3,clock_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(hash(10)).bind(credential.id).execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23502")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn live_credentials_and_sessions_protect_their_account(pool: PgPool) {
    let (owner, credential) = bootstrap(&pool).await;
    db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap()
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM management.accounts WHERE id=$1")
            .bind(owner)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM management.credentials WHERE id=$1")
            .bind(credential.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let actor = db::session_actor(&pool, &hash(10)).await.unwrap().unwrap();
    assert_eq!(actor.account_id, owner);
    assert_eq!(actor.credential_id, Some(credential.id));
}

#[sqlx::test(migrations = "./migrations")]
async fn schema_rejects_plaintext_hashes_and_audit_survives_physical_removal(pool: PgPool) {
    bootstrap(&pool).await;
    let target = user(&pool, Role::Reader).await;
    assert!(
        db::issue_credential(&pool, &context(), target, &key("raw-token"))
            .await
            .is_err()
    );
    let before = audit_count(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM management.accounts WHERE id=$1")
        .bind(target)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(audit_count(&pool).await, before);
    let target_in_audit: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.audit_events WHERE resource_id=$1)",
    )
    .bind(target.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(target_in_audit);
}
