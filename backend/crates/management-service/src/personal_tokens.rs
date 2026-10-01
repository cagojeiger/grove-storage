use filegate_core::SecretString;
use filegate_db::{PgPool, management as db};
use uuid::Uuid;

use crate::{Error, logging, password_setups};

pub async fn list(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    page: db::queries::Page<Uuid>,
) -> Result<Vec<db::queries::CredentialSummary>, Error> {
    logging::session_operation(
        pool,
        request_id,
        session_hash,
        "identity.credential.list",
        async {
            db::personal_tokens::list(pool, session_hash, page)
                .await
                .map_err(Error::from)
        },
    )
    .await
}

pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    current_password: SecretString,
    key: &db::NewCredential<'_>,
) -> Result<db::Credential, Error> {
    logging::session_operation(
        pool,
        request_id,
        session_hash,
        "identity.credential.issue",
        async {
            password_setups::reauthenticate(pool, session_hash, current_password).await?;
            db::personal_tokens::issue(pool, request_id, session_hash, key)
                .await
                .map_err(Error::from)
        },
    )
    .await
}

pub async fn revoke(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    credential: Uuid,
) -> Result<bool, Error> {
    logging::session_operation(
        pool,
        request_id,
        session_hash,
        "identity.credential.revoke",
        async {
            db::personal_tokens::revoke(pool, request_id, session_hash, credential)
                .await
                .map_err(Error::from)
        },
    )
    .await
}
