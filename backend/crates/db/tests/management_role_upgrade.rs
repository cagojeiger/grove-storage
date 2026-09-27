#![allow(clippy::unwrap_used)]
#[path = "support/management.rs"]
mod support;

use filegate_db::{PgPool, management as db};
use grove_management_policy::{AccountState, Actor, Role};
use sqlx::migrate::Migrate;
use support::*;
use uuid::Uuid;

#[sqlx::test(migrations = false)]
async fn role_rename_preserves_tokens_sessions_and_historical_role_snapshots(pool: PgPool) {
    let migrations = sqlx::migrate!("./migrations");
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in migrations.iter().filter(|m| m.version <= 12) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);

    let mut fixtures = Vec::new();
    for (index, (old, new, role)) in [
        ("viewer", "reader", Role::Reader),
        ("operator", "writer", Role::Writer),
        ("admin", "admin", Role::Admin),
    ]
    .into_iter()
    .enumerate()
    {
        let user = Uuid::new_v4();
        let credential = Uuid::new_v4();
        let token = hash(index as u64 + 1);
        let session = hash(index as u64 + 10);
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user','Existing user',$2)")
            .bind(user).bind(old).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO management.users(account_id) VALUES($1)")
            .bind(user)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at) VALUES($1,$2,'Existing token','gsm_old',$3,clock_timestamp()+interval '1 day')")
            .bind(credential).bind(user).bind(&token).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,credential_id,expires_at) VALUES($1,$2,'token',$3,$4,clock_timestamp()+interval '1 hour')")
            .bind(Uuid::new_v4()).bind(&session).bind(user).bind(credential).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO management.audit_events(actor_kind,actor_id,credential_id,request_id,surface,action,resource_type,resource_id,metadata) VALUES('user',$1,$2,$3,'console','account.role','account',$4,$5)")
            .bind(user).bind(credential).bind(Uuid::new_v4()).bind(user.to_string())
            .bind(serde_json::json!({"after_role":old})).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        fixtures.push((user, credential, token, session, new, role));
    }
    let before: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(t)::text FROM management.audit_events t ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(t)::text FROM management.audit_events t ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    for (user, credential, token, session, name, role) in fixtures {
        let stored: String = sqlx::query_scalar("SELECT role FROM management.accounts WHERE id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, name);
        for identity in [
            db::authenticate(&pool, &token).await.unwrap().unwrap(),
            db::session_actor(&pool, &session).await.unwrap().unwrap(),
        ] {
            assert_eq!(identity.account_id, user);
            assert_eq!(identity.credential_id, Some(credential));
            assert_eq!(
                identity.caller.actor,
                Actor::User {
                    role,
                    state: AccountState::Active
                }
            );
        }
        assert!(
            db::create_session(
                &pool,
                Uuid::new_v4(),
                &token,
                &Uuid::new_v4().simple().to_string().repeat(2)
            )
            .await
            .unwrap()
            .is_some()
        );
        for obsolete in ["viewer", "operator"] {
            assert!(
                sqlx::query("UPDATE management.accounts SET role=$1 WHERE id=$2")
                    .bind(obsolete)
                    .bind(user)
                    .execute(&pool)
                    .await
                    .is_err()
            );
        }
    }
}
