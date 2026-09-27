#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{PgPool, management as db};
use sqlx::migrate::Migrate;
use support::*;
use uuid::Uuid;

async fn snapshot(pool: &PgPool) -> Vec<Vec<serde_json::Value>> {
    let mut rows = Vec::new();
    for table in [
        "accounts",
        "credentials",
        "sessions",
        "audit_events",
        "command_invocations",
        "security_events",
        "root_sessions",
    ] {
        let projection = if table == "accounts" {
            "to_jsonb(t)-'user_id'"
        } else {
            "to_jsonb(t)"
        };
        rows.push(
            sqlx::query_scalar(&format!(
                "SELECT {projection} FROM management.{table} t ORDER BY id"
            ))
            .fetch_all(pool)
            .await
            .unwrap(),
        );
    }
    rows
}

#[sqlx::test(migrations = false)]
async fn account_simplification_preserves_data_and_authentication(pool: PgPool) {
    let migrations = sqlx::migrate!("./migrations");
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in migrations.iter().filter(|m| m.version <= 15) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);

    let owner = Uuid::new_v4();
    let credential = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    for (id, role, active, deleted) in [
        (owner, "admin", true, false),
        (Uuid::new_v4(), "writer", true, false),
        (Uuid::new_v4(), "reader", false, false),
        (Uuid::new_v4(), "reader", false, true),
    ] {
        sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role,is_active,deleted_at) VALUES($1,'user','Existing account',$2,$3,CASE WHEN $4 THEN clock_timestamp() END)")
            .bind(id).bind(role).bind(active).bind(deleted).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO management.users(account_id) VALUES($1)")
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at) VALUES($1,$2,'Existing token','gst_old',$3,clock_timestamp()+interval '1 day')")
        .bind(credential).bind(owner).bind(hash(1)).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let session = db::create_session(&pool, Uuid::new_v4(), &hash(1), &hash(10))
        .await
        .unwrap()
        .unwrap();
    let ctx = db::AuditContext {
        actor: db::AuditActor::User {
            id: owner,
            credential_id: credential,
            session_id: Some(session.id),
        },
        ..context()
    };
    db::telemetry::invocation(
        &pool,
        &ctx,
        "account.list",
        db::telemetry::Outcome::Succeeded,
        None,
        1,
    )
    .await
    .unwrap();
    db::telemetry::security(
        &pool,
        Some(&ctx),
        ctx.request_id,
        ctx.surface,
        db::telemetry::SecurityReason::Forbidden,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO management.root_sessions(id,session_hash,generation,expires_at) VALUES($1,$2,1,clock_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(hash(20)).execute(&pool).await.unwrap();
    let before = snapshot(&pool).await;

    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
    let users: Option<String> = sqlx::query_scalar("SELECT to_regclass('management.users')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(users.is_none());
    for actor in [
        db::authenticate(&pool, &hash(1)).await.unwrap().unwrap(),
        db::session_actor(&pool, &hash(10)).await.unwrap().unwrap(),
    ] {
        assert_eq!(actor.account_id, owner);
        assert_eq!(actor.credential_id, credential);
    }
    let created = user(&pool, grove_management_policy::Role::Reader).await;
    db::issue_credential(&pool, &context(), created, &key(&hash(2)))
        .await
        .unwrap();
    assert!(
        db::create_session(&pool, Uuid::new_v4(), &hash(2), &hash(11))
            .await
            .unwrap()
            .is_some()
    );
}
