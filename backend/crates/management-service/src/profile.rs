use filegate_db::{PgPool, management as db};
use uuid::Uuid;

use crate::Error;

pub async fn get(pool: &PgPool, session_hash: &str) -> Result<db::queries::AccountSummary, Error> {
    db::profile::get(pool, session_hash)
        .await
        .map_err(Error::from)
}

pub async fn rename(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    display_name: String,
) -> Result<bool, Error> {
    db::profile::rename(pool, request_id, session_hash, display_name)
        .await
        .map_err(Error::from)
}
