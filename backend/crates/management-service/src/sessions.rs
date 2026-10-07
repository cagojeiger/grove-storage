//! Console login. Browser admission is enforced by the HTTP boundary.
use crate::{Error, logging};
use grove_core::SecretString;
use grove_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, admission, telemetry},
};
use grove_management_policy::Surface;
use uuid::Uuid;

const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXJhbmRvbXNhbHQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

pub struct Login {
    pub request_id: Uuid,
    pub result: Result<db::Session, Error>,
}

/// Browser admission failures have no authenticated actor or invocation.
pub async fn reject_browser(pool: &PgPool, error: Error) -> Uuid {
    let request_id = Uuid::new_v4();
    let reason = if error == Error::Forbidden {
        telemetry::SecurityReason::Forbidden
    } else {
        telemetry::SecurityReason::Unauthenticated
    };
    logging::security(pool, None, request_id, Surface::Console, reason).await;
    request_id
}

pub async fn login(pool: &PgPool, token_hash: Option<&str>, session_hash: &str) -> Login {
    let request_id = Uuid::new_v4();
    let started = tokio::time::Instant::now();
    let result = exchange(pool, request_id, token_hash, session_hash).await;
    record_login(pool, request_id, started, result).await
}

pub async fn login_password(
    pool: &PgPool,
    username: &str,
    password: SecretString,
    session_hash: &str,
) -> Login {
    let request_id = Uuid::new_v4();
    let started = tokio::time::Instant::now();
    let result = password_exchange(pool, request_id, username, password, session_hash).await;
    record_login(pool, request_id, started, result).await
}

async fn record_login(
    pool: &PgPool,
    request_id: Uuid,
    started: tokio::time::Instant,
    result: Result<db::Session, Error>,
) -> Login {
    let context = result.as_ref().ok().map(|session| AuditContext {
        actor: AuditActor::Account {
            id: session.account_id,
            credential_id: session.credential_id,
            session_id: Some(session.id),
        },
        request_id,
        surface: Surface::Console,
    });
    logging::record(
        pool,
        context.as_ref(),
        request_id,
        Surface::Console,
        "identity.session.login",
        started.elapsed(),
        &result,
    )
    .await;
    if result.is_ok() {
        logging::security(
            pool,
            context.as_ref(),
            request_id,
            Surface::Console,
            telemetry::SecurityReason::Authenticated,
        )
        .await;
    }
    Login { request_id, result }
}

async fn password_exchange(
    pool: &PgPool,
    request_id: Uuid,
    username: &str,
    password: SecretString,
    session_hash: &str,
) -> Result<db::Session, Error> {
    let login = crate::passwords::username(username).ok();
    let stored = match login.as_deref() {
        Some(login) => db::passwords::find(pool, login)
            .await
            .map_err(Error::from)?,
        None => None,
    };
    let allowed = match &stored {
        Some(credential) => {
            admission::account_allowed(pool, credential.account_id, admission::Purpose::Login).await
        }
        None => admission::anonymous_allowed(pool).await,
    }
    .map_err(Error::from)?;
    if !allowed {
        return Err(Error::RateLimited);
    }
    let hash = stored
        .as_ref()
        .map_or(DUMMY_HASH, |row| row.password_hash.as_str());
    let verified = crate::passwords::verify(password, SecretString::from(hash.to_owned()))
        .await
        .map_err(crate::local_accounts::password_error)?;
    if !verified || stored.is_none() {
        return Err(Error::Unauthenticated);
    }
    let stored = stored.ok_or(Error::Unauthenticated)?;
    db::sessions::create_password_session(
        pool,
        request_id,
        stored.account_id,
        stored.generation,
        session_hash,
    )
    .await
    .map_err(Error::from)?
    .ok_or(Error::Unauthenticated)
}

async fn exchange(
    pool: &PgPool,
    request_id: Uuid,
    token_hash: Option<&str>,
    session_hash: &str,
) -> Result<db::Session, Error> {
    if !admission::anonymous_allowed(pool)
        .await
        .map_err(Error::from)?
    {
        return Err(Error::RateLimited);
    }
    let hash = token_hash.ok_or(Error::Unauthenticated)?;
    db::create_session(pool, request_id, hash, session_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)
}
