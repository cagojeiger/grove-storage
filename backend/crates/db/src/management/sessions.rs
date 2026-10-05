use chrono::{DateTime, Utc};
use grove_management_policy::{Actor, Surface};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{AuditActor, AuditContext, Error, audit, identity, lock};

#[derive(sqlx::FromRow, Debug)]
pub struct Session {
    pub id: Uuid,
    pub account_id: Uuid,
    pub credential_id: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
}

/// Raw secrets, login budgets, Origin and CSRF belong to service/HTTP callers.
/// Preserve the existing eight-hour, 64-per-credential session bounds.
pub async fn create_session(
    pool: &PgPool,
    request_id: Uuid,
    token_hash: &str,
    session_hash: &str,
) -> Result<Option<Session>, Error> {
    let mut tx = lock(pool).await?;
    let Some(actor) = identity::token(&mut tx, token_hash).await? else {
        return Ok(None);
    };
    if !matches!(actor.caller.actor, Actor::User { .. }) {
        return Ok(None);
    }
    let credential_id = actor.credential_id.ok_or(Error::InvalidInput)?;
    sqlx::query("DELETE FROM management.sessions WHERE credential_id=$1 AND (expires_at<=grove_time.wall_now() OR revoked_at IS NOT NULL)")
        .bind(credential_id).execute(&mut *tx).await?;
    let evicted: Vec<Uuid> = sqlx::query_scalar("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE id IN
        (SELECT id FROM management.sessions WHERE credential_id=$1 AND revoked_at IS NULL ORDER BY created_at DESC,id DESC OFFSET 63) RETURNING id")
        .bind(credential_id).fetch_all(&mut *tx).await?;
    let session: Session = sqlx::query_as("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,credential_id,expires_at)
        SELECT $1,$2,'token',account_id,id,LEAST(expires_at,grove_time.wall_now()+interval '8 hours') FROM management.credentials WHERE id=$3
        RETURNING id,account_id,credential_id,expires_at")
        .bind(Uuid::new_v4()).bind(session_hash).bind(credential_id).fetch_one(&mut *tx).await?;
    let context = AuditContext {
        actor: AuditActor::User {
            id: actor.account_id,
            credential_id: actor.credential_id,
            session_id: Some(session.id),
        },
        request_id,
        surface: Surface::Console,
    };
    for id in evicted {
        audit::record(&mut tx, &context, "session.evict", "session", id).await?;
    }
    audit::record(&mut tx, &context, "session.create", "session", session.id).await?;
    sqlx::query("UPDATE management.credentials SET last_used_at=grove_time.wall_now() WHERE id=$1")
        .bind(credential_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(Some(session))
}

/// Recheck the verified password generation while holding the identity lock.
/// A concurrent recovery/change wins either before login or after it, never
/// leaves a session authenticated by the old password usable.
pub async fn create_password_session(
    pool: &PgPool,
    request_id: Uuid,
    account: Uuid,
    generation: Uuid,
    session_hash: &str,
) -> Result<Option<Session>, Error> {
    let mut tx = lock(pool).await?;
    let current: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.password_credentials p
         JOIN management.accounts a ON a.id=p.account_id
         WHERE p.account_id=$1 AND p.generation=$2
         AND a.is_active AND a.deleted_at IS NULL)",
    )
    .bind(account)
    .bind(generation)
    .fetch_one(&mut *tx)
    .await?;
    if !current {
        return Ok(None);
    }
    sqlx::query("DELETE FROM management.sessions WHERE account_id=$1 AND auth_method='password' AND (expires_at<=grove_time.wall_now() OR revoked_at IS NOT NULL)")
        .bind(account).execute(&mut *tx).await?;
    let evicted: Vec<Uuid> = sqlx::query_scalar("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE id IN
        (SELECT id FROM management.sessions WHERE account_id=$1 AND auth_method='password' AND revoked_at IS NULL ORDER BY created_at DESC,id DESC OFFSET 63) RETURNING id")
        .bind(account).fetch_all(&mut *tx).await?;
    let session: Session = sqlx::query_as("INSERT INTO management.sessions(id,session_hash,auth_method,account_id,password_generation,expires_at)
        VALUES($1,$2,'password',$3,$4,grove_time.wall_now()+interval '8 hours')
        RETURNING id,account_id,credential_id,expires_at")
        .bind(Uuid::new_v4()).bind(session_hash).bind(account).bind(generation)
        .fetch_one(&mut *tx).await?;
    let context = AuditContext {
        actor: AuditActor::User {
            id: account,
            credential_id: None,
            session_id: Some(session.id),
        },
        request_id,
        surface: Surface::Console,
    };
    for id in evicted {
        audit::record(&mut tx, &context, "session.evict", "session", id).await?;
    }
    audit::record(&mut tx, &context, "session.create", "session", session.id).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(Some(session))
}

/// The authenticated Account ID constrains session ownership inside the mutation.
pub async fn revoke_session(
    pool: &PgPool,
    context: &AuditContext,
    account_id: Uuid,
    session_id: Uuid,
) -> Result<bool, Error> {
    revoke_in(lock(pool).await?, context, account_id, session_id).await
}

pub(super) async fn revoke_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    account_id: Uuid,
    session_id: Uuid,
) -> Result<bool, Error> {
    let changed = sqlx::query("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE id=$1 AND account_id=$2 AND revoked_at IS NULL")
        .bind(session_id).bind(account_id).execute(&mut *tx).await?.rows_affected() > 0;
    if changed {
        audit::record(&mut tx, context, "session.revoke", "session", session_id).await?;
    }
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(changed)
}
