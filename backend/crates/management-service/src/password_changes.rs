use grove_core::SecretString;
use grove_db::{
    PgPool,
    management::{self as db, admission},
};
use uuid::Uuid;

use crate::{Error, local_accounts::password_error, logging, passwords};

pub struct Change {
    pub request_id: Uuid,
    pub result: Result<(), Error>,
}

pub async fn change(
    pool: &PgPool,
    session_hash: &str,
    current: SecretString,
    replacement: SecretString,
) -> Change {
    let request_id = Uuid::new_v4();
    let result = logging::session_operation(
        pool,
        request_id,
        session_hash,
        "account.password_change",
        run(pool, request_id, session_hash, current, replacement),
    )
    .await;
    Change { request_id, result }
}

async fn run(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    current: SecretString,
    replacement: SecretString,
) -> Result<(), Error> {
    let identity = db::session_actor(pool, session_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)?;
    if !admission::account_allowed(
        pool,
        identity.account_id,
        admission::Purpose::Reauthentication,
    )
    .await
    .map_err(Error::from)?
    {
        return Err(Error::RateLimited);
    }
    let credential = db::passwords::find_by_account(pool, identity.account_id)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)?;
    let original_hash = SecretString::from(credential.password_hash);
    if !passwords::verify(current, original_hash.clone())
        .await
        .map_err(password_error)?
    {
        return Err(Error::Unauthenticated);
    }
    if passwords::verify(replacement.clone(), original_hash)
        .await
        .map_err(password_error)?
    {
        return Err(Error::InvalidInput);
    }
    let new_hash = passwords::hash(&credential.login_name, replacement)
        .await
        .map_err(password_error)?;
    grove_db::management::passwords::change(
        pool,
        request_id,
        session_hash,
        credential.generation,
        grove_core::ExposeSecret::expose_secret(&new_hash),
    )
    .await
    .map_err(Error::from)?
    .then_some(())
    .ok_or(Error::Unauthenticated)
}
