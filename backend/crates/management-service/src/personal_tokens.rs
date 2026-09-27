use filegate_core::SecretString;
use filegate_db::{PgPool, management as db};
use uuid::Uuid;

use crate::{Error, password_setups};

pub async fn list(
    pool: &PgPool,
    session_hash: &str,
    page: db::queries::Page<Uuid>,
) -> Result<Vec<db::queries::CredentialSummary>, Error> {
    db::personal_tokens::list(pool, session_hash, page)
        .await
        .map_err(Error::from)
}

pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    current_password: SecretString,
    key: &db::NewCredential<'_>,
) -> Result<db::Credential, Error> {
    password_setups::reauthenticate(pool, session_hash, current_password).await?;
    db::personal_tokens::issue(pool, request_id, session_hash, key)
        .await
        .map_err(Error::from)
}

pub async fn revoke(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    credential: Uuid,
) -> Result<bool, Error> {
    db::personal_tokens::revoke(pool, request_id, session_hash, credential)
        .await
        .map_err(Error::from)
}
