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
    sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user','orphan','reader')").bind(id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert!(sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'agent','rejected','reader')").bind(Uuid::new_v4()).execute(&pool).await.is_err());
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
async fn session_fk_rejects_cross_user_credentials(pool: PgPool) {
    let (owner, credential) = bootstrap(&pool).await;
    let other = user(&pool, Role::Reader).await;
    let automation_user = owner;
    let token_key = db::issue_credential(&pool, &context(), automation_user, &key(&hash(2)))
        .await
        .unwrap();
    for (user_id, credential_id) in [(other, credential.id), (other, token_key.id)] {
        assert!(sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,credential_id,expires_at) VALUES($1,$2,'token',$3,$4,clock_timestamp()+interval '1 hour')")
            .bind(Uuid::new_v4()).bind(hash(10)).bind(user_id).bind(credential_id).execute(&pool).await.is_err());
    }
    assert!(sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,credential_id,master_generation,expires_at) VALUES($1,$2,'master',$3,$4,'generation',clock_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(hash(10)).bind(owner).bind(credential.id).execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,expires_at) VALUES($1,$2,'master',clock_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(hash(10)).execute(&pool).await.is_err());
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
