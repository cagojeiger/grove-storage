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
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.credentials WHERE account_id=$1 AND revoked_at IS NULL AND expires_at>grove_time.wall_now()")
        .bind(account).fetch_one(&mut **tx).await?;
    if count >= 32 {
        return Err(Error::CredentialLimit);
    }
    Ok(sqlx::query_as(
        "INSERT INTO management.credentials(id,account_id,label,token_prefix,token_hash,expires_at)
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
    audit::record(
        &mut tx,
        context,
        "credential.issue",
        "credential",
        credential.id,
    )
    .await?;
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
    let changed = sqlx::query("UPDATE management.credentials SET revoked_at=grove_time.wall_now() WHERE id=$1 AND revoked_at IS NULL")
        .bind(id).execute(&mut *tx).await?.rows_affected() > 0;
    if changed {
        sqlx::query("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE credential_id=$1 AND revoked_at IS NULL")
            .bind(id).execute(&mut *tx).await?;
        audit::record(&mut tx, context, "credential.revoke", "credential", id).await?;
    }
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(changed)
}

/// Legacy token recovery preserves other Users and Client credentials.
pub async fn recover_admin(
    pool: &PgPool,
    context: &AuditContext,
    account: Uuid,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    recover_in(lock(pool).await?, context, account, key).await
}

pub(super) async fn recover_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    account: Uuid,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    let admin: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts WHERE id=$1 AND role='admin' AND is_active AND deleted_at IS NULL)")
        .bind(account).fetch_one(&mut *tx).await?;
    if !admin {
        return Err(Error::NotFound);
    }
    sqlx::query("UPDATE management.credentials SET revoked_at=grove_time.wall_now() WHERE account_id=$1 AND revoked_at IS NULL")
        .bind(account).execute(&mut *tx).await?;
    sqlx::query("UPDATE management.sessions SET revoked_at=grove_time.wall_now() WHERE account_id=$1 AND revoked_at IS NULL")
        .bind(account).execute(&mut *tx).await?;
    let credential = insert(&mut tx, account, key).await?;
    audit::record(&mut tx, context, "user.recover", "account", account).await?;
    audit::record(
        &mut tx,
        context,
        "credential.issue",
        "credential",
        credential.id,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(credential)
}
