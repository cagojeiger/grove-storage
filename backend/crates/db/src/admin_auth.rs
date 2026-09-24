//! Administrator credentials are independent of client and provider credentials.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct Credential {
    pub id: Uuid,
    pub label: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

// Serializes local credential mutations, including the first initialization.
async fn lock(pool: &PgPool) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(5064092085313881166)")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub async fn initialized(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_principals)")
        .fetch_one(pool)
        .await
}

pub enum IssueMode {
    Initialize,
    Create,
    Recover,
}

/// None means initialization state does not permit the requested operation.
pub async fn issue(
    pool: &PgPool,
    mode: IssueMode,
    label: &str,
    hash: &str,
) -> Result<Option<Credential>, sqlx::Error> {
    let mut tx = lock(pool).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_principals)")
        .fetch_one(&mut *tx)
        .await?;
    let action = match mode {
        IssueMode::Initialize if !exists => {
            sqlx::query("INSERT INTO admin_principals(id) VALUES(1)")
                .execute(&mut *tx)
                .await?;
            "initialize"
        }
        IssueMode::Create if exists => "credential.create",
        IssueMode::Recover if exists => {
            sqlx::query("UPDATE admin_credentials SET revoked_at = now() WHERE revoked_at IS NULL")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM admin_sessions")
                .execute(&mut *tx)
                .await?;
            "recover"
        }
        _ => return Ok(None),
    };
    let credential = sqlx::query_as::<_, Credential>(
        "INSERT INTO admin_credentials(id, principal_id, label, token_hash, expires_at)
         VALUES($1, 1, $2, $3, now() + interval '90 days')
         RETURNING id, label, expires_at, revoked_at",
    )
    .bind(Uuid::new_v4())
    .bind(label)
    .bind(hash)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO admin_audit_events(actor, action, target, status)
                 VALUES('local', $1, $2, 200)",
    )
    .bind(action)
    .bind(credential.id.to_string())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(credential))
}

pub async fn list(pool: &PgPool) -> Result<Vec<Credential>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, label, expires_at, revoked_at FROM admin_credentials ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
}

pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let mut tx = lock(pool).await?;
    let changed = sqlx::query(
        "UPDATE admin_credentials SET revoked_at = COALESCE(revoked_at, now()) WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;
    sqlx::query("DELETE FROM admin_sessions WHERE credential_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if changed {
        sqlx::query(
            "INSERT INTO admin_audit_events(actor, action, target, status)
                     VALUES('local', 'credential.revoke', $1, 200)",
        )
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(changed)
}

pub async fn authenticate(pool: &PgPool, hash: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM admin_credentials WHERE token_hash = $1
                        AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await
}

pub async fn login_allowed(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("UPDATE admin_login_budget SET
        attempts = CASE WHEN window_start <= now() - interval '1 minute' THEN 1 ELSE attempts + 1 END,
        window_start = CASE WHEN window_start <= now() - interval '1 minute' THEN now() ELSE window_start END
        WHERE id = 1 AND (attempts < 60 OR window_start <= now() - interval '1 minute')
        RETURNING true")
        .fetch_optional(pool).await.map(|v| v.unwrap_or(false))
}

/// Shares the credential lock with revocation, so login cannot leave a live session behind.
pub async fn create_session(
    pool: &PgPool,
    token_hash: &str,
    session_hash: &str,
) -> Result<Option<(Uuid, DateTime<Utc>)>, sqlx::Error> {
    let mut tx = lock(pool).await?;
    sqlx::query("DELETE FROM admin_sessions WHERE expires_at <= now()")
        .execute(&mut *tx)
        .await?;
    let credential: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM admin_credentials WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()")
        .bind(token_hash).fetch_optional(&mut *tx).await?;
    let Some(id) = credential else {
        return Ok(None);
    };
    // Keep at most 64 sessions per credential, evicting the oldest on login.
    sqlx::query(
        "DELETE FROM admin_sessions WHERE session_hash IN
        (SELECT session_hash FROM admin_sessions WHERE credential_id = $1
         ORDER BY created_at DESC, session_hash OFFSET 63)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let result = sqlx::query_as("INSERT INTO admin_sessions(session_hash, credential_id, expires_at)
        SELECT $1, id, LEAST(expires_at, now() + interval '8 hours') FROM admin_credentials WHERE id = $2
        RETURNING credential_id, expires_at")
        .bind(session_hash).bind(id).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO admin_audit_events(credential_id, actor, action, target, status)
                 VALUES($1, 'admin', 'session.create', 'session', 200)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(result))
}

pub async fn session_actor(pool: &PgPool, hash: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT c.id FROM admin_sessions s JOIN admin_credentials c ON c.id = s.credential_id
        WHERE s.session_hash = $1 AND s.expires_at > now() AND c.expires_at > now() AND c.revoked_at IS NULL")
        .bind(hash).fetch_optional(pool).await
}

pub async fn logout(pool: &PgPool, hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_sessions WHERE session_hash = $1")
        .bind(hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn audit_start(
    pool: &PgPool,
    credential: Option<Uuid>,
    action: &str,
    target: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO admin_audit_events(credential_id, actor, action, target)
        VALUES($1, $2, $3, $4) RETURNING id",
    )
    .bind(credential)
    .bind(if credential.is_some() {
        "admin"
    } else {
        "legacy"
    })
    .bind(action)
    .bind(target)
    .fetch_one(pool)
    .await
}

pub async fn audit_finish(pool: &PgPool, id: i64, status: u16) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE admin_audit_events SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(i32::from(status))
        .execute(pool)
        .await?;
    Ok(())
}
