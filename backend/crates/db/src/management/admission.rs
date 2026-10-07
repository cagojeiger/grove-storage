//! Authentication admission is enforced before hashing and fails closed on DB errors.
use super::Error;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum Purpose {
    Login,
    Reauthentication,
}

/// At most two rows per registered account; arbitrary login names create no rows.
pub async fn account_allowed(
    pool: &PgPool,
    account: Uuid,
    purpose: Purpose,
) -> Result<bool, Error> {
    let purpose = match purpose {
        Purpose::Login => "login",
        Purpose::Reauthentication => "reauthentication",
    };
    Ok(sqlx::query_scalar(
        "INSERT INTO management.authentication_budgets AS budget (account_id,purpose,attempts)
         SELECT id,$2,1 FROM management.accounts WHERE id=$1 AND is_active AND deleted_at IS NULL
         ON CONFLICT (account_id,purpose) DO UPDATE SET
           attempts=CASE WHEN budget.window_start<=grove_time.wall_now()-interval '1 minute'
                         THEN 1 ELSE budget.attempts+1 END,
           window_start=CASE WHEN budget.window_start<=grove_time.wall_now()-interval '1 minute'
                             THEN grove_time.wall_now() ELSE budget.window_start END
         WHERE budget.attempts<60 OR budget.window_start<=grove_time.wall_now()-interval '1 minute'
         RETURNING true",
    )
    .bind(account)
    .bind(purpose)
    .fetch_optional(pool)
    .await?
    .unwrap_or(false))
}

/// Fixed-size fallback for unknown usernames.
pub async fn anonymous_allowed(pool: &PgPool) -> Result<bool, Error> {
    Ok(sqlx::query_scalar("UPDATE management.login_budget SET
        attempts=CASE WHEN window_start<=grove_time.wall_now()-interval '1 minute' THEN 1 ELSE attempts+1 END,
        window_start=CASE WHEN window_start<=grove_time.wall_now()-interval '1 minute' THEN grove_time.wall_now() ELSE window_start END
        WHERE id=1 AND (attempts<60 OR window_start<=grove_time.wall_now()-interval '1 minute') RETURNING true")
        .fetch_optional(pool).await?.unwrap_or(false))
}
