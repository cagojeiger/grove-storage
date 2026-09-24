use grove_management_policy::{AccountState, Actor, AuthMethod, Caller, CredentialState, Role};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::Error;

#[derive(Debug)]
pub struct Identity {
    pub account_id: Uuid,
    pub credential_id: Uuid,
    pub session_id: Option<Uuid>,
    pub caller: Caller,
}

#[derive(sqlx::FromRow)]
struct Row {
    account_id: Uuid,
    kind: String,
    role: String,
    credential_id: Uuid,
    session_id: Option<Uuid>,
}

fn role(value: &str) -> Result<Role, Error> {
    match value {
        "viewer" => Ok(Role::Viewer),
        "operator" => Ok(Role::Operator),
        "admin" => Ok(Role::Admin),
        _ => Err(Error::InvalidInput),
    }
}

impl Row {
    fn identity(self, method: AuthMethod) -> Result<Identity, Error> {
        let actor = match self.kind.as_str() {
            "user" => Actor::User {
                role: role(&self.role)?,
                state: AccountState::Active,
            },
            _ => return Err(Error::InvalidInput),
        };
        Ok(Identity {
            account_id: self.account_id,
            credential_id: self.credential_id,
            session_id: self.session_id,
            caller: Caller {
                actor,
                method,
                credential_state: CredentialState::Active,
            },
        })
    }
}

pub(super) async fn token(
    connection: &mut PgConnection,
    hash: &str,
) -> Result<Option<Identity>, Error> {
    let row: Option<Row> = sqlx::query_as("SELECT a.id AS account_id,a.kind,a.role,c.id AS credential_id,NULL::uuid AS session_id
        FROM management.credentials c JOIN management.accounts a ON a.id=c.account_id
        WHERE c.token_hash=$1 AND c.hash_version=1 AND c.revoked_at IS NULL AND c.expires_at>clock_timestamp()
        AND a.is_active AND a.deleted_at IS NULL AND a.kind='user'")
        .bind(hash).fetch_optional(connection).await?;
    row.map(|row| row.identity(AuthMethod::ManagementToken))
        .transpose()
}

/// Every lookup reads current token and User state.
pub async fn authenticate(pool: &PgPool, hash: &str) -> Result<Option<Identity>, Error> {
    token(&mut *pool.acquire().await?, hash).await
}

pub async fn session_actor(pool: &PgPool, hash: &str) -> Result<Option<Identity>, Error> {
    session(&mut *pool.acquire().await?, hash).await
}

pub(super) async fn session(
    connection: &mut PgConnection,
    hash: &str,
) -> Result<Option<Identity>, Error> {
    let row: Option<Row> = sqlx::query_as("SELECT a.id AS account_id,a.kind,a.role,c.id AS credential_id,s.id AS session_id
        FROM management.sessions s JOIN management.credentials c ON c.id=s.credential_id AND c.account_id=s.user_id
        JOIN management.accounts a ON a.id=s.user_id
        WHERE s.session_hash=$1 AND s.auth_method='token' AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp()
        AND c.hash_version=1 AND c.revoked_at IS NULL AND c.expires_at>clock_timestamp()
        AND a.kind='user' AND a.is_active AND a.deleted_at IS NULL")
        .bind(hash).fetch_optional(connection).await?;
    row.map(|row| row.identity(AuthMethod::UserSession))
        .transpose()
}
