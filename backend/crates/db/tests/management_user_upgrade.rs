#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{PgPool, management as db};
use sqlx::migrate::Migrate;
use support::*;
use uuid::Uuid;

#[sqlx::test(migrations = false)]
async fn unification_preserves_ids_history_and_user_sessions_without_promoting_agent_tokens(
    pool: PgPool,
) {
    let migrations = sqlx::migrate!("./migrations");
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in migrations.iter().filter(|m| m.version <= 11) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);
    // Seed the old schema directly; current writers target the latest schema.
    let owner = Uuid::new_v4();
    let original = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user','Owner','admin')")
        .bind(owner).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO management.users(account_id) VALUES($1)")
        .bind(owner)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at) VALUES($1,$2,'Owner token','gst_old',$3,clock_timestamp()+interval '1 day')")
        .bind(original).bind(owner).bind(hash(1)).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,credential_id,expires_at) VALUES($1,$2,'token',$3,$4,clock_timestamp()+interval '8 hours')")
        .bind(Uuid::new_v4()).bind(hash(10)).bind(owner).bind(original).execute(&pool).await.unwrap();
    let agent = Uuid::new_v4();
    let credential = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'agent','old automation','operator')")
        .bind(agent).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO management.agents(account_id,owner_user_id) VALUES($1,$2)")
        .bind(agent)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at) VALUES($1,$2,'old job','gsm_old',$3,clock_timestamp()+interval '1 day')")
        .bind(credential).bind(agent).bind(hash(2)).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO management.audit_events(actor_kind,actor_id,owner_user_id,credential_id,request_id,surface,action,resource_type,resource_id) VALUES('agent',$1,$2,$3,$4,'mcp','storage.create','storage','kept')")
        .bind(agent).bind(owner).bind(credential).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let before: String = sqlx::query_scalar(
        "SELECT row_to_json(t)::text FROM management.audit_events t WHERE actor_id=$1",
    )
    .bind(agent)
    .fetch_one(&pool)
    .await
    .unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    let row: (bool, String) =
        sqlx::query_as("SELECT is_active,role FROM management.accounts WHERE id=$1")
            .bind(agent)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, (false, "writer".into()));
    let after: String = sqlx::query_scalar(
        "SELECT row_to_json(t)::text FROM management.audit_events t WHERE actor_id=$1",
    )
    .bind(agent)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        db::authenticate(&pool, &hash(1))
            .await
            .unwrap()
            .unwrap()
            .credential_id,
        Some(original)
    );
    assert!(db::session_actor(&pool, &hash(10)).await.unwrap().is_some());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
    db::change_account(&pool, &context(), agent, db::AccountChange::Active(true))
        .await
        .unwrap();
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(2), &hash(11))
            .await
            .unwrap()
            .is_none()
    );
    let fresh = db::issue_credential(&pool, &context(), agent, &key(&hash(3)))
        .await
        .unwrap();
    assert_eq!(
        db::authenticate(&pool, &hash(3))
            .await
            .unwrap()
            .unwrap()
            .credential_id,
        Some(fresh.id)
    );
}
