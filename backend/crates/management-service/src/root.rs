//! Root console login does not accept or promote an existing setup session.
use crate::{Error, logging, master::Config};
use filegate_db::{PgPool, management as db};
use grove_management_policy::Surface;
use uuid::Uuid;

pub async fn login(
    pool: &PgPool,
    config: &Config,
    presented: Option<&str>,
    session_hash: &str,
) -> crate::master::Login {
    let request_id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let result = async {
        if !db::telemetry::login_allowed(pool).await? {
            return Err(Error::RateLimited);
        }
        if !config.matches(presented) {
            return Err(Error::Unauthenticated);
        }
        db::IdentityTransaction::begin(pool)
            .await?
            .create_root_session(config.binding(), session_hash, request_id)
            .await
            .map_err(Error::from)
    }
    .await;
    let context = result
        .as_ref()
        .ok()
        .map(|s| db::master::context(request_id, s.id));
    logging::record(
        pool,
        context.as_ref(),
        request_id,
        Surface::Console,
        "root.session.login",
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
            db::telemetry::SecurityReason::Authenticated,
        )
        .await;
    }
    crate::master::Login { request_id, result }
}
