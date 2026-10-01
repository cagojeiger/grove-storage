use chrono::{DateTime, Utc};
use grove_management_policy::{Actor, Role, Surface};
use sqlx::PgPool;
use uuid::Uuid;

use super::{AuditActor, AuditContext, Error, NewAccount, audit, identity, lock, role_name};

pub struct Setup {
    pub account_id: Uuid,
    pub login_name: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    account: NewAccount<'_>,
    login_name: &str,
    token_hash: &str,
) -> Result<Setup, Error> {
    let mut tx = lock(pool).await?;
    let context = admin_password_session(&mut tx, request_id, session_hash).await?;
    let account_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user',$2,$3)",
    )
    .bind(account_id)
    .bind(account.display_name)
    .bind(role_name(account.role))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO management.password_credentials(account_id,login_name,password_hash,generation,password_changed_at)
         VALUES($1,$2,NULL,NULL,NULL)",
    )
    .bind(account_id)
    .bind(login_name)
    .execute(&mut *tx)
    .await?;
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO management.password_setup_tokens(account_id,token_hash,expires_at)
         VALUES($1,$2,clock_timestamp()+interval '24 hours') RETURNING expires_at",
    )
    .bind(account_id)
    .bind(token_hash)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(&mut tx, &context, "account.create", "account", account_id).await?;
    audit::record(
        &mut tx,
        &context,
        "account.password_setup.issue",
        "account",
        account_id,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(Setup {
        account_id,
        login_name: login_name.to_owned(),
        expires_at,
    })
}

async fn admin_password_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: Uuid,
    session_hash: &str,
) -> Result<AuditContext, Error> {
    let actor = identity::password_session(tx, session_hash).await?;
    let actor_session = actor.session_id.ok_or(Error::InvalidInput)?;
    if !matches!(
        actor.caller.actor,
        Actor::User {
            role: Role::Admin,
            ..
        }
    ) {
        return Err(Error::Forbidden);
    }
    Ok(AuditContext {
        actor: AuditActor::User {
            id: actor.account_id,
            credential_id: actor.credential_id,
            session_id: Some(actor_session),
        },
        request_id,
        surface: Surface::Console,
    })
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    kind: String,
    is_active: bool,
    deleted_at: Option<DateTime<Utc>>,
    login_name: Option<String>,
    password_hash: Option<String>,
}

/// Issuance is bound to a current Admin password session in the same transaction.
pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    account_id: Uuid,
    login_name: &str,
    token_hash: &str,
) -> Result<Setup, Error> {
    let mut tx = lock(pool).await?;
    let context = admin_password_session(&mut tx, request_id, session_hash).await?;
    let account: Option<AccountRow> = sqlx::query_as(
            "SELECT a.kind,a.is_active,a.deleted_at,p.login_name,p.password_hash
             FROM management.accounts a LEFT JOIN management.password_credentials p ON p.account_id=a.id
             WHERE a.id=$1",
        )
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(account) = account else {
        return Err(Error::NotFound);
    };
    if account.kind != "user" || !account.is_active || account.deleted_at.is_some() {
        return Err(Error::InactiveAccount);
    }
    if account.password_hash.is_some() {
        return Err(Error::AlreadyInitialized);
    }
    if let Some(reserved_name) = account.login_name {
        if reserved_name != login_name {
            return Err(Error::InvalidInput);
        }
    } else {
        sqlx::query(
            "INSERT INTO management.password_credentials(account_id,login_name,password_hash,generation,password_changed_at)
             VALUES($1,$2,NULL,NULL,NULL)",
        )
        .bind(account_id)
        .bind(login_name)
        .execute(&mut *tx)
        .await?;
    }
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO management.password_setup_tokens(account_id,token_hash,expires_at)
         VALUES($1,$2,clock_timestamp()+interval '24 hours')
         ON CONFLICT(account_id) DO UPDATE SET token_hash=EXCLUDED.token_hash,
         created_at=clock_timestamp(),expires_at=clock_timestamp()+interval '24 hours'
         RETURNING expires_at",
    )
    .bind(account_id)
    .bind(token_hash)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &context,
        "account.password_setup.issue",
        "account",
        account_id,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(Setup {
        account_id,
        login_name: login_name.to_owned(),
        expires_at,
    })
}

pub async fn inspect(pool: &PgPool, token_hash: &str) -> Result<Option<Setup>, Error> {
    let row = sqlx::query_as(
        "SELECT t.account_id,p.login_name,t.expires_at
         FROM management.password_setup_tokens t
         JOIN management.password_credentials p ON p.account_id=t.account_id
         JOIN management.accounts a ON a.id=t.account_id
         WHERE t.token_hash=$1 AND t.expires_at>clock_timestamp()
           AND p.password_hash IS NULL AND a.kind='user'
           AND a.is_active AND a.deleted_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(account_id, login_name, expires_at)| Setup {
        account_id,
        login_name,
        expires_at,
    }))
}

pub async fn complete(
    pool: &PgPool,
    request_id: Uuid,
    token_hash: &str,
    password_hash: &str,
) -> Result<Option<Uuid>, Error> {
    let mut tx = lock(pool).await?;
    let account: Option<Uuid> = sqlx::query_scalar(
        "SELECT t.account_id FROM management.password_setup_tokens t
         JOIN management.password_credentials p ON p.account_id=t.account_id
         JOIN management.accounts a ON a.id=t.account_id
         WHERE t.token_hash=$1 AND t.expires_at>clock_timestamp()
           AND p.password_hash IS NULL AND a.kind='user'
           AND a.is_active AND a.deleted_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(account) = account else {
        return Ok(None);
    };
    sqlx::query(
        "UPDATE management.password_credentials
         SET password_hash=$2,generation=$3,password_changed_at=clock_timestamp()
         WHERE account_id=$1 AND password_hash IS NULL",
    )
    .bind(account)
    .bind(password_hash)
    .bind(Uuid::new_v4())
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM management.password_setup_tokens WHERE account_id=$1")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    let context = AuditContext {
        actor: AuditActor::User {
            id: account,
            credential_id: None,
            session_id: None,
        },
        request_id,
        surface: Surface::Console,
    };
    audit::record(
        &mut tx,
        &context,
        "account.password_setup.complete",
        "account",
        account,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(Some(account))
}
