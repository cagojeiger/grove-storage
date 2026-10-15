use grove_core::SecretString;
use grove_db::{PgPool, management as db};
use grove_management_policy::{Actor, Role};
use uuid::Uuid;

use crate::{Error, local_accounts::password_error, logging, passwords};

pub async fn issue(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    account: Uuid,
    username: &str,
    current_password: SecretString,
    token_hash: &str,
) -> Result<db::password_setup::Setup, Error> {
    let mut details = logging::Details::new(
        "identity.account.password_setup.issue",
        logging::Stage::Authentication,
    );
    details.target = Some(logging::Target::account(account));
    logging::session_operation(pool, request_id, session_hash, details, async |details| {
        details.stage = logging::Stage::Validation;
        let username = passwords::username(username).map_err(password_error)?;
        reauthenticate_admin(pool, session_hash, current_password, request_id, details).await?;
        details.stage = logging::Stage::Execution;
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
    })
    .await
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
    logging::session_operation(
        pool,
        request_id,
        session_hash,
        logging::Details::new("identity.account.create", logging::Stage::Authentication),
        async |details| {
            details.stage = logging::Stage::Validation;
            let username = passwords::username(username).map_err(password_error)?;
            reauthenticate_admin(pool, session_hash, current_password, request_id, details).await?;
            details.stage = logging::Stage::Execution;
            let setup = db::password_setup::create(
                pool,
                request_id,
                session_hash,
                account,
                &username,
                token_hash,
            )
            .await
            .map_err(Error::from)?;
            details.target = Some(logging::Target::account(setup.account_id));
            Ok(setup)
        },
    )
    .await
}

async fn reauthenticate_admin(
    pool: &PgPool,
    session_hash: &str,
    current_password: SecretString,
    request_id: Uuid,
    details: &mut logging::Details,
) -> Result<(), Error> {
    let actor = reauthenticate(pool, session_hash, current_password, request_id, details).await?;
    details.stage = logging::Stage::Authorization;
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
    request_id: Uuid,
    details: &mut logging::Details,
) -> Result<db::Identity, Error> {
    details.stage = logging::Stage::Authentication;
    let actor = db::session_actor(pool, session_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)?;
    details.stage = logging::Stage::Reauthentication;
    details.auth_method = Some(logging::AuthMethod::Password);
    if !db::admission::account_allowed(
        pool,
        actor.account_id,
        db::admission::Purpose::Reauthentication,
    )
    .await
    .map_err(Error::from)?
    {
        return Err(Error::RateLimited);
    }
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
    logging::reauthenticated(pool, request_id, &actor, details).await;
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
        grove_core::ExposeSecret::expose_secret(&password_hash),
    )
    .await
    .map_err(Error::from)?
    .map(|_| ())
    .ok_or(Error::NotFound)
}
