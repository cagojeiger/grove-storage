//! Server-local account provisioning. These functions are deliberately outside
//! the remotely callable command catalog and require server/DB operator access.
use filegate_core::{ExposeSecret, SecretString};
use filegate_db::{PgPool, management::passwords as db};
use uuid::Uuid;

use crate::{Error, passwords};

pub async fn initialize(
    pool: &PgPool,
    request_id: Uuid,
    username: &str,
    display_name: &str,
    password: SecretString,
) -> Result<Uuid, Error> {
    let username = passwords::username(username).map_err(password_error)?;
    let name = display_name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(Error::InvalidInput);
    }
    let hash = passwords::hash(&username, password)
        .await
        .map_err(password_error)?;
    db::initialize(pool, request_id, &username, name, hash.expose_secret())
        .await
        .map_err(Into::into)
}

pub async fn recover(
    pool: &PgPool,
    request_id: Uuid,
    account_id: Uuid,
    username: &str,
    password: SecretString,
) -> Result<(), Error> {
    let username = passwords::username(username).map_err(password_error)?;
    let hash = passwords::hash(&username, password)
        .await
        .map_err(password_error)?;
    db::recover(
        pool,
        request_id,
        account_id,
        &username,
        hash.expose_secret(),
    )
    .await
    .map_err(Into::into)
}

pub(crate) fn password_error(error: passwords::Error) -> Error {
    match error {
        passwords::Error::InvalidUsername
        | passwords::Error::InvalidLength
        | passwords::Error::WeakPassword => Error::InvalidInput,
        passwords::Error::Busy => Error::RateLimited,
        passwords::Error::InvalidHash | passwords::Error::Unavailable => Error::Unavailable,
    }
}
