use super::{Error, IdentityTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Private bounds make every query bounded, including internal callers.
#[derive(Clone, Copy)]
pub struct Page<K> {
    pub(super) before: Option<K>,
    pub(super) limit: i64,
}
impl<K> Page<K> {
    pub fn new(before: Option<K>, limit: u16) -> Result<Self, Error> {
        if !(1..=100).contains(&limit) {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            before,
            limit: i64::from(limit),
        })
    }
}
impl<K> Default for Page<K> {
    fn default() -> Self {
        Self {
            before: None,
            limit: 50,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct AccountSummary {
    pub id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}
#[derive(Debug, sqlx::FromRow)]
pub struct CredentialSummary {
    pub id: Uuid,
    pub account_id: Uuid,
    pub label: String,
    pub token_prefix: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Debug, sqlx::FromRow)]
pub struct SessionSummary {
    pub id: Uuid,
    pub credential_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl IdentityTransaction<'_> {
    pub async fn account(&mut self, id: Uuid) -> Result<AccountSummary, Error> {
        sqlx::query_as(
            "SELECT id,kind,display_name,role,is_active,deleted_at
            FROM management.accounts WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&mut *self.inner)
        .await?
        .ok_or(Error::NotFound)
    }

    pub async fn accounts(&mut self, page: Page<Uuid>) -> Result<Vec<AccountSummary>, Error> {
        Ok(sqlx::query_as(
            "SELECT a.id,a.kind,a.display_name,a.role,a.is_active,a.deleted_at
            FROM management.accounts a
            WHERE ($1::uuid IS NULL OR a.id<$1) ORDER BY a.id DESC LIMIT $2",
        )
        .bind(page.before)
        .bind(page.limit)
        .fetch_all(&mut *self.inner)
        .await?)
    }
    pub async fn credentials(
        &mut self,
        account: Uuid,
        page: Page<Uuid>,
    ) -> Result<Vec<CredentialSummary>, Error> {
        Ok(sqlx::query_as("SELECT id,account_id,label,token_prefix,created_at,expires_at,revoked_at FROM management.credentials
            WHERE account_id=$1 AND ($2::uuid IS NULL OR id<$2) ORDER BY id DESC LIMIT $3")
            .bind(account).bind(page.before).bind(page.limit).fetch_all(&mut *self.inner).await?)
    }
    pub async fn sessions(
        &mut self,
        user: Uuid,
        page: Page<Uuid>,
    ) -> Result<Vec<SessionSummary>, Error> {
        Ok(sqlx::query_as(
            "SELECT id,credential_id,created_at,expires_at,revoked_at FROM management.sessions
            WHERE user_id=$1 AND ($2::uuid IS NULL OR id<$2) ORDER BY id DESC LIMIT $3",
        )
        .bind(user)
        .bind(page.before)
        .bind(page.limit)
        .fetch_all(&mut *self.inner)
        .await?)
    }
}
