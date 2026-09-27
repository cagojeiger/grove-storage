use super::{Error, IdentityTransaction};
use chrono::{DateTime, Utc};
use grove_management_policy::Role;
use uuid::Uuid;

#[derive(Default, Clone, Copy)]
pub enum AccountStatus {
    #[default]
    All,
    Current,
    Active,
    Disabled,
    Deleted,
}

pub struct AccountQuery {
    page: Page<Uuid>,
    after: Option<Uuid>,
    search: String,
    role: Option<Role>,
    status: AccountStatus,
}

impl From<Page<Uuid>> for AccountQuery {
    fn from(page: Page<Uuid>) -> Self {
        Self {
            page,
            after: None,
            search: String::new(),
            role: None,
            status: AccountStatus::All,
        }
    }
}

impl AccountQuery {
    pub fn new(
        page: Page<Uuid>,
        after: Option<Uuid>,
        search: String,
        role: Option<Role>,
        status: AccountStatus,
    ) -> Result<Self, Error> {
        if (page.before.is_some() && after.is_some()) || search.chars().count() > 80 {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            page,
            after,
            search: search.trim().to_owned(),
            role,
            status,
        })
    }
}

#[derive(Debug)]
pub struct AccountPage {
    pub items: Vec<AccountSummary>,
    pub next_before: Option<Uuid>,
    pub previous_after: Option<Uuid>,
    pub initialized: bool,
}

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

    pub async fn accounts(&mut self, query: AccountQuery) -> Result<AccountPage, Error> {
        let status = match query.status {
            AccountStatus::All => "all",
            AccountStatus::Current => "current",
            AccountStatus::Active => "active",
            AccountStatus::Disabled => "disabled",
            AccountStatus::Deleted => "deleted",
        };
        let mut rows: Vec<AccountSummary> = sqlx::query_as(
            "SELECT a.id,a.kind,a.display_name,a.role,a.is_active,a.deleted_at
            FROM management.accounts a
            WHERE ($1::uuid IS NULL OR a.id<$1) AND ($2::uuid IS NULL OR a.id>$2)
            AND ($3='' OR strpos(lower(a.display_name),lower($3))>0 OR strpos(a.id::text,lower($3))>0)
            AND ($4::text IS NULL OR a.role=$4)
            AND ($5='all' OR ($5='current' AND a.deleted_at IS NULL)
                OR ($5='active' AND a.deleted_at IS NULL AND a.is_active)
                OR ($5='disabled' AND a.deleted_at IS NULL AND NOT a.is_active)
                OR ($5='deleted' AND a.deleted_at IS NOT NULL))
            ORDER BY CASE WHEN $2::uuid IS NOT NULL THEN a.id END ASC, a.id DESC LIMIT $6",
        )
        .bind(query.page.before)
        .bind(query.after)
        .bind(query.search)
        .bind(query.role.map(super::role_name))
        .bind(status)
        .bind(query.page.limit + 1)
        .fetch_all(&mut *self.inner)
        .await?;
        // The extra row detects the page boundary; backward reads return in display order.
        let more = rows.len() > query.page.limit as usize;
        rows.truncate(query.page.limit as usize);
        if query.after.is_some() {
            rows.reverse();
        }
        let next_before = if query.after.is_some() || more {
            rows.last().map(|row| row.id).or(query.after)
        } else {
            None
        };
        let previous_after = if query.page.before.is_some() || (query.after.is_some() && more) {
            rows.first().map(|row| row.id).or(query.page.before)
        } else {
            None
        };
        let initialized = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
            .fetch_one(&mut *self.inner)
            .await?;
        Ok(AccountPage {
            items: rows,
            next_before,
            previous_after,
            initialized,
        })
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
