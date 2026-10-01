use grove_management_policy::Surface;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    AccountChange, AuditActor, AuditContext, Error, accounts, identity, lock,
    queries::{self, AccountSummary},
};

pub async fn get(pool: &PgPool, session_hash: &str) -> Result<AccountSummary, Error> {
    let mut tx = lock(pool).await?;
    let actor = identity::password_session(&mut tx, session_hash).await?;
    queries::account_in(&mut tx, actor.account_id).await
}

pub async fn rename(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    display_name: String,
) -> Result<bool, Error> {
    let mut tx = lock(pool).await?;
    let actor = identity::password_session(&mut tx, session_hash).await?;
    let context = AuditContext {
        actor: AuditActor::User {
            id: actor.account_id,
            credential_id: None,
            session_id: actor.session_id,
        },
        request_id,
        surface: Surface::Console,
    };
    accounts::change_in(
        tx,
        &context,
        actor.account_id,
        AccountChange::Name(display_name),
    )
    .await
}
