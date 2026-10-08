use grove_management_policy::Surface;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{
    AuditActor, AuditContext, Credential, Error, NewCredential, api_tokens, identity, lock,
    queries::{CredentialSummary, Page},
};

async fn password_session(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    session_hash: &str,
) -> Result<(Uuid, AuditContext), Error> {
    let actor = identity::password_session(tx, session_hash).await?;
    let session_id = actor.session_id.ok_or(Error::Unauthenticated)?;
    let context = AuditContext {
        actor: AuditActor::Account {
            id: actor.account_id,
            credential_id: None,
            session_id: Some(session_id),
        },
        request_id,
        surface: Surface::Console,
    };
    Ok((actor.account_id, context))
}

pub async fn list(
    pool: &PgPool,
    session_hash: &str,
    page: Page<Uuid>,
) -> Result<Vec<CredentialSummary>, Error> {
    let mut tx = lock(pool).await?;
    let (account, _) = password_session(&mut tx, Uuid::new_v4(), session_hash).await?;
    let rows = sqlx::query_as(
        "SELECT id,account_id,label,token_prefix,created_at,expires_at,revoked_at
         FROM management.api_tokens
         WHERE account_id=$1 AND ($2::uuid IS NULL OR id<$2)
         ORDER BY id DESC LIMIT $3",
    )
    .bind(account)
    .bind(page.before)
    .bind(page.limit)
    .fetch_all(&mut *tx)
    .await?;
    Ok(rows)
}

pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    key: &NewCredential<'_>,
) -> Result<Credential, Error> {
    let mut tx = lock(pool).await?;
    let (account, context) = password_session(&mut tx, request_id, session_hash).await?;
    let issued = api_tokens::insert(&mut tx, account, key).await?;
    api_tokens::record_audit(&mut tx, &context, "credential.issue", issued.id).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(issued)
}

pub async fn revoke(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    credential: Uuid,
) -> Result<bool, Error> {
    let mut tx = lock(pool).await?;
    let (account, context) = password_session(&mut tx, request_id, session_hash).await?;
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.api_tokens WHERE id=$1 AND account_id=$2)",
    )
    .bind(credential)
    .bind(account)
    .fetch_one(&mut *tx)
    .await?;
    if !owned {
        return Err(Error::NotFound);
    }
    api_tokens::revoke_in(tx, &context, credential).await
}
