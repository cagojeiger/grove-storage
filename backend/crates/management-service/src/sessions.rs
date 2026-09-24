//! User-token exchange only. The HTTP boundary supplies hashes after browser
//! checks; malformed tokens still consume the shared login budget.
use crate::{Error, logging};
use filegate_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, telemetry},
};
use grove_management_policy::Surface;
use uuid::Uuid;

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
    let started = std::time::Instant::now();
    let result = exchange(pool, request_id, token_hash, session_hash).await;
    let context = result.as_ref().ok().map(|session| AuditContext {
        actor: AuditActor::User {
            id: session.user_id,
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

async fn exchange(
    pool: &PgPool,
    request_id: Uuid,
    token_hash: Option<&str>,
    session_hash: &str,
) -> Result<db::Session, Error> {
    if !telemetry::login_allowed(pool).await.map_err(Error::from)? {
        return Err(Error::RateLimited);
    }
    let hash = token_hash.ok_or(Error::Unauthenticated)?;
    db::create_session(pool, request_id, hash, session_hash)
        .await
        .map_err(Error::from)?
        .ok_or(Error::Unauthenticated)
}
