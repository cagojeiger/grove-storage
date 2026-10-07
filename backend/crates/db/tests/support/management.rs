#![allow(dead_code, clippy::unwrap_used)]

use chrono::{Duration, Utc};
use grove_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, NewCredential},
};
use grove_management_policy::Surface;
use uuid::Uuid;

pub fn hash(value: u64) -> String {
    format!("{value:064x}")
}

pub fn context() -> AuditContext {
    AuditContext {
        actor: AuditActor::System,
        request_id: Uuid::new_v4(),
        surface: Surface::Console,
    }
}

pub fn key(hash: &str) -> NewCredential<'_> {
    NewCredential {
        label: "test",
        token_prefix: "gst_test",
        token_hash: hash,
        expires_at: Utc::now() + Duration::hours(1),
    }
}

pub async fn bootstrap(pool: &PgPool) -> (Uuid, db::Credential) {
    seed_admin(pool, &context(), "Owner", &key(&hash(1)))
        .await
        .unwrap()
}

pub async fn seed_admin(
    pool: &PgPool,
    context: &AuditContext,
    name: &str,
    key: &NewCredential<'_>,
) -> Result<(Uuid, db::Credential), db::Error> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO management.accounts(id,display_name,role) VALUES($1,$2,'admin')")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO management.audit_events(actor_kind,request_id,surface,action,resource_type,resource_id)
        VALUES('system',$1,'console','account.create','account',$2)")
        .bind(context.request_id).bind(id.to_string()).execute(pool).await?;
    let credential = db::issue_credential(pool, context, id, key).await?;
    Ok((id, credential))
}

pub async fn audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM management.audit_events")
        .fetch_one(pool)
        .await
        .unwrap()
}

pub async fn reject_audit(pool: &PgPool) {
    sqlx::raw_sql("CREATE FUNCTION management.reject_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected audit failure'; END $$;
        CREATE TRIGGER reject_audit BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_audit();")
        .execute(pool).await.unwrap();
}

pub async fn user(pool: &PgPool, role: grove_management_policy::Role) -> Uuid {
    db::create_account(
        pool,
        &context(),
        db::NewAccount {
            display_name: "User",
            role,
        },
    )
    .await
    .unwrap()
}
