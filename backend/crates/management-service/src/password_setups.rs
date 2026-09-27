use filegate_core::SecretString;
use filegate_db::{PgPool, management as db};
use grove_management_policy::{Actor, Role};
use uuid::Uuid;

use crate::{Error, local_accounts::password_error, passwords};

pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    account: Uuid,
    username: &str,
    current_password: SecretString,
    token_hash: &str,
) -> Result<db::password_setup::Setup, Error> {
    let username = passwords::username(username).map_err(password_error)?;
    reauthenticate_admin(pool, session_hash, current_password).await?;
    db::password_setup::issue(
        pool,
        request_id,
        session_hash,
        account,
        &username,
        token_hash,
    )
    .await
    .map_err(Error::from)
}

pub async fn create(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    account: db::NewAccount<'_>,
    username: &str,
    current_password: SecretString,
    token_hash: &str,
) -> Result<db::password_setup::Setup, Error> {
    let username = passwords::username(username).map_err(password_error)?;
    reauthenticate_admin(pool, session_hash, current_password).await?;
    db::password_setup::create(
        pool,
        request_id,
        session_hash,
        account,
        &username,
        token_hash,
    )
    .await
    .map_err(Error::from)
}

async fn reauthenticate_admin(
    pool: &PgPool,
    session_hash: &str,
    current_password: SecretString,
) -> Result<(), Error> {
    let actor = reauthenticate(pool, session_hash, current_password).await?;
    if !matches!(
        actor.caller.actor,
        Actor::User {
            role: Role::Admin,
            ..
        }
    ) {
        return Err(Error::Forbidden);
    }
    Ok(())
}

pub(crate) async fn reauthenticate(
    pool: &PgPool,
    session_hash: &str,
    current_password: SecretString,
) -> Result<db::Identity, Error> {
    if !db::telemetry::login_allowed(pool)
        .await
        .map_err(Error::from)?
    {
        return Err(Error::RateLimited);
    }
    let actor = db::session_actor(pool, session_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)?;
    let credential = db::passwords::find_by_account(pool, actor.account_id)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Forbidden)?;
    if !passwords::verify(
        current_password,
        SecretString::from(credential.password_hash),
    )
    .await
    .map_err(password_error)?
    {
        return Err(Error::Unauthenticated);
    }
    Ok(actor)
}

pub async fn inspect(pool: &PgPool, token_hash: &str) -> Result<db::password_setup::Setup, Error> {
    db::password_setup::inspect(pool, token_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::NotFound)
}

pub async fn complete(
    pool: &PgPool,
    request_id: Uuid,
    token_hash: &str,
    password: SecretString,
) -> Result<(), Error> {
    let setup = inspect(pool, token_hash).await?;
    let password_hash = passwords::hash(&setup.login_name, password)
        .await
        .map_err(password_error)?;
    db::password_setup::complete(
        pool,
        request_id,
        token_hash,
        filegate_core::ExposeSecret::expose_secret(&password_hash),
    )
    .await
    .map_err(Error::from)?
    .map(|_| ())
    .ok_or(Error::NotFound)
}
