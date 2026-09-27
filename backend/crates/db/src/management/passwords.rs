//! Privileged local provisioning primitives, not an HTTP authentication surface.
//! The service supplies validated names and PHC hashes; plaintext stays outside DB.
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{Error, lock};

pub struct PasswordCredential {
    pub account_id: Uuid,
    pub login_name: String,
    pub password_hash: String,
    pub generation: Uuid,
}

pub async fn find(pool: &PgPool, login: &str) -> Result<Option<PasswordCredential>, Error> {
    let row: Option<(Uuid, String, String, Uuid)> = sqlx::query_as(
        "SELECT p.account_id,p.login_name,p.password_hash,p.generation
         FROM management.password_credentials p JOIN management.accounts a ON a.id=p.account_id
         WHERE p.login_name=$1 AND p.password_hash IS NOT NULL
           AND a.is_active AND a.deleted_at IS NULL",
    )
    .bind(login)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(account_id, login_name, password_hash, generation)| PasswordCredential {
            account_id,
            login_name,
            password_hash,
            generation,
        },
    ))
}

pub async fn find_by_account(
    pool: &PgPool,
    account: Uuid,
) -> Result<Option<PasswordCredential>, Error> {
    let row: Option<(Uuid, String, String, Uuid)> = sqlx::query_as(
        "SELECT p.account_id,p.login_name,p.password_hash,p.generation
         FROM management.password_credentials p JOIN management.accounts a ON a.id=p.account_id
         WHERE p.account_id=$1 AND p.password_hash IS NOT NULL
           AND a.is_active AND a.deleted_at IS NULL",
    )
    .bind(account)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(account_id, login_name, password_hash, generation)| PasswordCredential {
            account_id,
            login_name,
            password_hash,
            generation,
        },
    ))
}

/// Password verification and hashing happen before this serialized mutation.
pub async fn change(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    verified_generation: Uuid,
    new_hash: &str,
) -> Result<bool, Error> {
    let mut tx = lock(pool).await?;
    let current: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT s.id,s.user_id FROM management.sessions s
         JOIN management.accounts a ON a.id=s.user_id
         JOIN management.password_credentials p ON p.account_id=a.id
         WHERE s.session_hash=$1 AND s.auth_method='password'
           AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp()
           AND s.password_generation=$2 AND p.generation=$2
           AND a.is_active AND a.deleted_at IS NULL",
    )
    .bind(session_hash)
    .bind(verified_generation)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((session, account)) = current else {
        return Ok(false);
    };
    let changed = sqlx::query(
        "UPDATE management.password_credentials
         SET password_hash=$2,generation=$3,password_changed_at=clock_timestamp()
         WHERE account_id=$1 AND generation=$4",
    )
    .bind(account)
    .bind(new_hash)
    .bind(Uuid::new_v4())
    .bind(verified_generation)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Ok(false);
    }
    sqlx::query("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL")
        .bind(account).execute(&mut *tx).await?;
    let context = super::AuditContext {
        actor: super::AuditActor::User {
            id: account,
            credential_id: None,
            session_id: Some(session),
        },
        request_id,
        surface: grove_management_policy::Surface::Console,
    };
    super::audit::record(
        &mut tx,
        &context,
        "account.password_change",
        "account",
        account,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(true)
}

pub async fn initialize(
    pool: &PgPool,
    request_id: Uuid,
    login: &str,
    display_name: &str,
    password_hash: &str,
) -> Result<Uuid, Error> {
    let mut tx = lock(pool).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        return Err(Error::AlreadyInitialized);
    }
    let account = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user',$2,'admin')",
    )
    .bind(account)
    .bind(display_name)
    .execute(&mut *tx)
    .await?;
    replace(&mut tx, account, login, password_hash).await?;
    local_audit(&mut tx, request_id, account, "account.initialize").await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(account)
}

/// Server operator recovery preserves the account's current role and identity.
pub async fn recover(
    pool: &PgPool,
    request_id: Uuid,
    account: Uuid,
    login: &str,
    password_hash: &str,
) -> Result<(), Error> {
    let mut tx = lock(pool).await?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.accounts WHERE id=$1 AND is_active AND deleted_at IS NULL)",
    ).bind(account).fetch_one(&mut *tx).await?;
    if !active {
        return Err(Error::InactiveAccount);
    }
    let existing_login: Option<String> = sqlx::query_scalar(
        "SELECT login_name FROM management.password_credentials WHERE account_id=$1",
    )
    .bind(account)
    .fetch_optional(&mut *tx)
    .await?;
    if existing_login.is_some_and(|existing| existing != login) {
        return Err(Error::InvalidInput);
    }
    replace(&mut tx, account, login, password_hash).await?;
    sqlx::query("DELETE FROM management.password_setup_tokens WHERE account_id=$1")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL")
        .bind(account).execute(&mut *tx).await?;
    sqlx::query("UPDATE management.credentials SET revoked_at=clock_timestamp() WHERE account_id=$1 AND revoked_at IS NULL")
        .bind(account).execute(&mut *tx).await?;
    local_audit(&mut tx, request_id, account, "account.password_recover").await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(())
}

async fn replace(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    login: &str,
    hash: &str,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO management.password_credentials(account_id,login_name,password_hash,generation)
         VALUES($1,$2,$3,$4) ON CONFLICT(account_id) DO UPDATE
         SET login_name=EXCLUDED.login_name,password_hash=EXCLUDED.password_hash,
             generation=EXCLUDED.generation,password_changed_at=clock_timestamp()",
    ).bind(account).bind(login).bind(hash).bind(Uuid::new_v4()).execute(&mut **tx).await?;
    Ok(())
}

async fn local_audit(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    account: Uuid,
    action: &str,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO management.audit_events(actor_kind,request_id,surface,action,resource_type,resource_id)
         VALUES('system',$1,'local',$2,'account',$3)",
    ).bind(request_id).bind(action).bind(account.to_string()).execute(&mut **tx).await?;
    Ok(())
}
