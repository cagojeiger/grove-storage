#![allow(clippy::unwrap_used)]

use filegate_db::{PgPool, management as db};
use serde_json::Value;
use sqlx::migrate::Migrate;
use uuid::Uuid;

async fn old_schema(pool: &PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in sqlx::migrate!("./migrations")
        .iter()
        .filter(|m| m.version <= 23)
    {
        connection.apply(migration).await.unwrap();
    }
}

async fn rows(pool: &PgPool, table: &str, projection: &str) -> Vec<Value> {
    sqlx::query_scalar(&format!(
        "SELECT {projection} FROM management.{table} t ORDER BY 1"
    ))
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = false)]
async fn cleanup_preserves_current_authentication_and_historical_actor_snapshots(pool: PgPool) {
    old_schema(&pool).await;
    let account = Uuid::new_v4();
    let credential = Uuid::new_v4();
    let generation = Uuid::new_v4();
    let token_session = Uuid::new_v4();
    let password_session = Uuid::new_v4();
    let master_session = Uuid::new_v4();
    sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user','Owner','admin')")
        .bind(account).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at) VALUES($1,$2,'existing','gsm_old',repeat('1',64),grove_time.wall_now()+interval '1 day')")
        .bind(credential).bind(account).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.password_credentials(account_id,login_name,password_hash,generation) VALUES($1,'owner','$argon2id$v=19$m=65536,t=3,p=1$existing$existing',$2)")
        .bind(account).bind(generation).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,credential_id,expires_at) VALUES($1,repeat('2',64),'token',$2,$3,grove_time.wall_now()+interval '1 hour')")
        .bind(token_session).bind(account).bind(credential).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,user_id,password_generation,expires_at) VALUES($1,repeat('3',64),'password',$2,$3,grove_time.wall_now()+interval '1 hour')")
        .bind(password_session).bind(account).bind(generation).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.sessions(id,session_hash,auth_method,master_generation,expires_at) VALUES($1,repeat('4',64),'master','old-generation',grove_time.wall_now()+interval '1 hour')")
        .bind(master_session).execute(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO management.root_sessions(id,session_hash,generation,expires_at) VALUES(gen_random_uuid(),repeat('5',64),1,grove_time.wall_now()+interval '1 hour');
        INSERT INTO management.master_configuration VALUES(1,1,repeat('6',64));")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.audit_events(actor_kind,session_id,request_id,surface,action,resource_type,resource_id) VALUES('master',$1,$2,'console','user.bootstrap','account',$3)")
        .bind(master_session).bind(Uuid::new_v4()).bind(account.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.audit_events(actor_kind,actor_id,owner_user_id,request_id,surface,action,resource_type,resource_id) VALUES('agent',$1,$2,$3,'mcp','storage.create','storage','historical')")
        .bind(Uuid::new_v4()).bind(account).bind(Uuid::new_v4()).execute(&pool).await.unwrap();

    let accounts = rows(&pool, "accounts", "to_jsonb(t)-'kind'").await;
    let credentials = rows(&pool, "credentials", "to_jsonb(t)").await;
    let passwords = rows(&pool, "password_credentials", "to_jsonb(t)").await;
    let audit = rows(&pool, "audit_events", "to_jsonb(t)").await;
    let sessions: Vec<Value> = sqlx::query_scalar(
        "SELECT (to_jsonb(t)-'master_generation'-'user_id') || jsonb_build_object('account_id',user_id)
         FROM management.sessions t WHERE auth_method <> 'master' ORDER BY 1")
        .fetch_all(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();
    filegate_db::migrate(&pool).await.unwrap();

    assert_eq!(rows(&pool, "accounts", "to_jsonb(t)").await, accounts);
    assert_eq!(rows(&pool, "credentials", "to_jsonb(t)").await, credentials);
    assert_eq!(
        rows(&pool, "password_credentials", "to_jsonb(t)").await,
        passwords
    );
    assert_eq!(rows(&pool, "audit_events", "to_jsonb(t)").await, audit);
    assert_eq!(rows(&pool, "sessions", "to_jsonb(t)").await, sessions);
    for table in ["root_sessions", "master_configuration"] {
        let name: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(format!("management.{table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(name.is_none());
    }
    assert_eq!(
        db::authenticate(&pool, &"1".repeat(64))
            .await
            .unwrap()
            .unwrap()
            .account_id,
        account
    );
    for (hash, session) in [
        ("2".repeat(64), token_session),
        ("3".repeat(64), password_session),
    ] {
        let actor = db::session_actor(&pool, &hash).await.unwrap().unwrap();
        assert_eq!(actor.account_id, account);
        assert_eq!(actor.session_id, Some(session));
    }
    assert!(
        db::session_actor(&pool, &"4".repeat(64))
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = false)]
async fn cleanup_failure_rolls_back_obsolete_session_deletion(pool: PgPool) {
    old_schema(&pool).await;
    sqlx::raw_sql("INSERT INTO management.sessions(id,session_hash,auth_method,master_generation,expires_at)
        VALUES(gen_random_uuid(),repeat('4',64),'master','old-generation',grove_time.wall_now()+interval '1 hour');
        ALTER TABLE management.sessions ADD COLUMN account_id uuid;")
        .execute(&pool).await.unwrap();
    let before = rows(&pool, "sessions", "to_jsonb(t)").await;
    assert!(filegate_db::migrate(&pool).await.is_err());
    assert_eq!(rows(&pool, "sessions", "to_jsonb(t)").await, before);
    for table in ["root_sessions", "master_configuration"] {
        let name: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(format!("management.{table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(name.is_some());
    }
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(latest, 23);
}
