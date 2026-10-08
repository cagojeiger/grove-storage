use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{AuditContext, Error, audit, lock};

// Only verification material crosses this repository boundary, never raw tokens.
pub struct NewCredential<'a> {
    pub label: &'a str,
    pub token_prefix: &'a str,
    pub token_hash: &'a str,
    pub expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Debug)]
pub struct Credential {
    pub id: Uuid,
    pub account_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub(super) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.accounts a
        WHERE a.id=$1 AND a.is_active AND a.deleted_at IS NULL
       )",
    )
    .bind(account)
    .fetch_one(&mut **tx)
    .await?;
    if !active {
        return Err(Error::InactiveAccount);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.api_tokens WHERE account_id=$1 AND revoked_at IS NULL AND expires_at>grove_time.wall_now()")
        .bind(account).fetch_one(&mut **tx).await?;
    if count >= 32 {
        return Err(Error::CredentialLimit);
    }
    Ok(sqlx::query_as(
        "INSERT INTO management.api_tokens(id,account_id,label,token_prefix,token_hash,expires_at)
        VALUES($1,$2,$3,$4,$5,$6) RETURNING id,account_id,expires_at",
    )
    .bind(Uuid::new_v4())
    .bind(account)
    .bind(key.label)
    .bind(key.token_prefix)
    .bind(key.token_hash)
    .bind(key.expires_at)
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn issue_credential(
    pool: &PgPool,
    context: &AuditContext,
    account: Uuid,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    issue_in(lock(pool).await?, context, account, key).await
}

pub(super) async fn issue_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    account: Uuid,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    let credential = insert(&mut tx, account, key).await?;
    record_audit(&mut tx, context, "credential.issue", credential.id).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(credential)
}

pub async fn revoke_credential(
    pool: &PgPool,
    context: &AuditContext,
    id: Uuid,
) -> Result<bool, Error> {
    revoke_in(lock(pool).await?, context, id).await
}

pub(super) async fn revoke_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    id: Uuid,
) -> Result<bool, Error> {
    let changed = sqlx::query("UPDATE management.api_tokens SET revoked_at=grove_time.wall_now() WHERE id=$1 AND revoked_at IS NULL")
        .bind(id).execute(&mut *tx).await?.rows_affected() > 0;
    if changed {
        sqlx::query("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE credential_id=$1 AND revoked_at IS NULL")
            .bind(id).execute(&mut *tx).await?;
        record_audit(&mut tx, context, "credential.revoke", id).await?;
    }
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(changed)
}

pub(super) async fn record_audit(
    tx: &mut Transaction<'_, Postgres>,
    context: &AuditContext,
    action: &str,
    credential: Uuid,
) -> Result<(), Error> {
    let event = audit::record(tx, context, action, "credential", credential).await?;
    sqlx::query(
        "UPDATE management.audit_events e SET metadata = jsonb_build_object(
            'account_id', t.account_id, 'label', t.label,
            'expires_at', t.expires_at)
         FROM management.api_tokens t WHERE e.id=$1 AND t.id=$2",
    )
    .bind(event)
    .bind(credential)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
