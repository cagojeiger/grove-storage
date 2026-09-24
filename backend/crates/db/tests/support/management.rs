#![allow(dead_code, clippy::unwrap_used)]

use chrono::{Duration, Utc};
use filegate_db::{
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
        actor: AuditActor::Master { session_id: None },
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
    db::bootstrap(pool, &context(), "Owner", &key(&hash(1)))
        .await
        .unwrap()
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
        db::NewAccount::User {
            display_name: "User",
            role,
        },
    )
    .await
    .unwrap()
}

pub async fn agent(pool: &PgPool, owner: Uuid) -> Uuid {
    db::create_account(
        pool,
        &context(),
        db::NewAccount::Agent {
            display_name: "Agent",
            role: grove_management_policy::AgentRole::Operator,
            owner_user_id: owner,
        },
    )
    .await
    .unwrap()
}
