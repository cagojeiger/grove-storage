use grove_management_policy::Role;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{AuditContext, Credential, Error, NewCredential, audit, credentials, lock, role_name};

pub struct NewAccount<'a> {
    pub display_name: &'a str,
    pub role: Role,
}

pub enum AccountChange {
    Name(String),
    Role(Role),
    Active(bool),
    Delete,
}

/// Called only after master setup authorization. Account and first key are atomic.
pub async fn bootstrap(
    pool: &PgPool,
    context: &AuditContext,
    name: &str,
    key: &NewCredential<'_>,
) -> Result<(Uuid, Credential), Error> {
    bootstrap_in(lock(pool).await?, context, name, key).await
}

pub(super) async fn bootstrap_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    name: &str,
    key: &NewCredential<'_>,
) -> Result<(Uuid, Credential), Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        return Err(Error::AlreadyInitialized);
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,'user',$2,'admin')",
    )
    .bind(id)
    .bind(name)
    .execute(&mut *tx)
    .await?;
    let credential = credentials::insert(&mut tx, id, key).await?;
    audit::record(&mut tx, context, "user.bootstrap", "account", id).await?;
    audit::record(
        &mut tx,
        context,
        "credential.issue",
        "credential",
        credential.id,
    )
    .await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok((id, credential))
}

pub async fn create_account(
    pool: &PgPool,
    context: &AuditContext,
    account: NewAccount<'_>,
) -> Result<Uuid, Error> {
    create_in(lock(pool).await?, context, account).await
}

pub(super) async fn create_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    account: NewAccount<'_>,
) -> Result<Uuid, Error> {
    let name = account.display_name;
    let role = role_name(account.role);
    let initialized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
        .fetch_one(&mut *tx)
        .await?;
    if !initialized {
        return Err(Error::InvalidInput);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO management.accounts(id,kind,display_name,role) VALUES($1,$2,$3,$4)")
        .bind(id)
        .bind("user")
        .bind(name)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    audit::record(&mut tx, context, "account.create", "account", id).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(id)
}

/// Returns false for an idempotent change; no duplicate audit is written.
pub async fn change_account(
    pool: &PgPool,
    context: &AuditContext,
    id: Uuid,
    change: AccountChange,
) -> Result<bool, Error> {
    change_in(lock(pool).await?, context, id, change).await
}

pub(super) async fn change_in(
    mut tx: Transaction<'_, Postgres>,
    context: &AuditContext,
    id: Uuid,
    change: AccountChange,
) -> Result<bool, Error> {
    let row: Option<(String, String, bool, String)> = sqlx::query_as(
        "SELECT kind,role,is_active,display_name FROM management.accounts WHERE id=$1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((kind, old_role, old_active, old_name)) = row else {
        return Err(Error::NotFound);
    };
    let (role, active, deleted, action) = match change {
        AccountChange::Name(name) => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 80 {
                return Err(Error::InvalidInput);
            }
            if name == old_name {
                return Ok(false);
            }
            sqlx::query("UPDATE management.accounts SET display_name=$2,updated_at=clock_timestamp() WHERE id=$1")
                .bind(id).bind(name).execute(&mut *tx).await?;
            let event = audit::record(&mut tx, context, "account.name", "account", id).await?;
            sqlx::query("UPDATE management.audit_events SET metadata=jsonb_build_object('before_name',$2::text,'after_name',$3::text) WHERE id=$1")
                .bind(event).bind(old_name).bind(name).execute(&mut *tx).await?;
            tx.commit().await.map_err(|_| Error::CommitUnknown)?;
            return Ok(true);
        }
        AccountChange::Role(role) => (role_name(role), old_active, false, "account.role"),
        AccountChange::Active(active) => (old_role.as_str(), active, false, "account.active"),
        AccountChange::Delete => (old_role.as_str(), false, true, "account.delete"),
    };
    if role == old_role && active == old_active && !deleted {
        return Ok(false);
    }
    if kind == "user" && old_role == "admin" && old_active && (role != "admin" || !active) {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts WHERE kind='user' AND role='admin' AND is_active AND deleted_at IS NULL").fetch_one(&mut *tx).await?;
        if count <= 1 {
            return Err(Error::LastAdmin);
        }
    }
    sqlx::query("UPDATE management.accounts SET role=$2,is_active=$3,deleted_at=CASE WHEN $4 THEN clock_timestamp() END,updated_at=clock_timestamp() WHERE id=$1")
        .bind(id).bind(role).bind(active).bind(deleted).execute(&mut *tx).await?;
    if !active {
        // Old sessions stay revoked after reactivation.
        sqlx::query("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL").bind(id).execute(&mut *tx).await?;
    }
    if deleted {
        sqlx::query("UPDATE management.credentials SET revoked_at=clock_timestamp() WHERE revoked_at IS NULL AND account_id=$1")
            .bind(id).execute(&mut *tx).await?;
    }
    let event = audit::record(&mut tx, context, action, "account", id).await?;
    sqlx::query("UPDATE management.audit_events SET metadata=jsonb_build_object('before_role',$2::text,'after_role',$3::text,'before_active',$4::boolean,'after_active',$5::boolean,'deleted',$6::boolean) WHERE id=$1")
        .bind(event).bind(&old_role).bind(role).bind(old_active).bind(active).bind(deleted).execute(&mut *tx).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(true)
}
