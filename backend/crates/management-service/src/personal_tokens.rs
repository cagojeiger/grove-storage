use grove_core::SecretString;
use grove_db::{PgPool, management as db};
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
        logging::Details::new("identity.credential.list", logging::Stage::Authentication),
        async |_| {
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
        logging::Details::new("identity.credential.issue", logging::Stage::Authentication),
        async |details| {
            password_setups::reauthenticate(
                pool,
                session_hash,
                current_password,
                request_id,
                details,
            )
            .await?;
            details.stage = logging::Stage::Execution;
            let credential = db::personal_tokens::issue(pool, request_id, session_hash, key)
                .await
                .map_err(Error::from)?;
            details.target = Some(logging::Target::credential(credential.id));
            Ok(credential)
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
    let mut details =
        logging::Details::new("identity.credential.revoke", logging::Stage::Authentication);
    details.target = Some(logging::Target::credential(credential));
    logging::session_operation(pool, request_id, session_hash, details, async |_| {
        db::personal_tokens::revoke(pool, request_id, session_hash, credential)
            .await
            .map_err(Error::from)
    })
    .await
}
