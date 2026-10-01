use super::{Error, IdentityTransaction, queries::Page};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Actor filters narrow the authorized scope; they do not select mutation targets.
#[derive(Default)]
pub struct HistoryQuery {
    pub page: Page<i64>,
    pub account_id: Option<Uuid>,
    pub credential_id: Option<Uuid>,
}
impl From<Page<i64>> for HistoryQuery {
    fn from(page: Page<i64>) -> Self {
        Self {
            page,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy)]
pub enum HistoryScope {
    Installation,
    // Includes immutable legacy Agent-owner snapshots for pre-unification events.
    User(Uuid),
}
impl HistoryScope {
    fn user(self) -> Option<Uuid> {
        match self {
            Self::Installation => None,
            Self::User(id) => Some(id),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventContext {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub actor_kind: String,
    pub actor_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub credential_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub request_id: Uuid,
    pub surface: String,
}
#[derive(Debug, sqlx::FromRow)]
pub struct AuditEvent {
    #[sqlx(flatten)]
    pub context: EventContext,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub metadata: serde_json::Value,
}
#[derive(Debug, sqlx::FromRow)]
pub struct Invocation {
    #[sqlx(flatten)]
    pub context: EventContext,
    pub operation: String,
    pub outcome: String,
    pub error_code: Option<String>,
    pub duration_ms: i64,
}
#[derive(Debug, sqlx::FromRow)]
pub struct SecurityEvent {
    #[sqlx(flatten)]
    pub context: EventContext,
    pub event_type: String,
    pub reason_code: String,
}

impl IdentityTransaction<'_> {
    pub async fn audit_history(
        &mut self,
        scope: HistoryScope,
        query: HistoryQuery,
    ) -> Result<Vec<AuditEvent>, Error> {
        Ok(sqlx::query_as("SELECT * FROM management.audit_events WHERE ($1::uuid IS NULL OR actor_id=$1 OR owner_user_id=$1)
            AND ($2::bigint IS NULL OR id<$2)
            AND ($4::uuid IS NULL OR actor_id=$4 OR owner_user_id=$4)
            AND ($5::uuid IS NULL OR credential_id=$5) ORDER BY id DESC LIMIT $3")
            .bind(scope.user()).bind(query.page.before).bind(query.page.limit)
            .bind(query.account_id).bind(query.credential_id).fetch_all(&mut *self.inner).await?)
    }
    pub async fn invocation_history(
        &mut self,
        scope: HistoryScope,
        query: HistoryQuery,
    ) -> Result<Vec<Invocation>, Error> {
        Ok(sqlx::query_as("SELECT * FROM management.command_invocations WHERE ($1::uuid IS NULL OR actor_id=$1 OR owner_user_id=$1)
            AND ($2::bigint IS NULL OR id<$2)
            AND ($4::uuid IS NULL OR actor_id=$4 OR owner_user_id=$4)
            AND ($5::uuid IS NULL OR credential_id=$5) ORDER BY id DESC LIMIT $3")
            .bind(scope.user()).bind(query.page.before).bind(query.page.limit)
            .bind(query.account_id).bind(query.credential_id).fetch_all(&mut *self.inner).await?)
    }
    pub async fn security_history(
        &mut self,
        query: HistoryQuery,
    ) -> Result<Vec<SecurityEvent>, Error> {
        Ok(sqlx::query_as(
            "SELECT * FROM management.security_events WHERE ($1::bigint IS NULL OR id<$1)
            AND ($3::uuid IS NULL OR actor_id=$3 OR owner_user_id=$3)
            AND ($4::uuid IS NULL OR credential_id=$4) ORDER BY id DESC LIMIT $2",
        )
        .bind(query.page.before)
        .bind(query.page.limit)
        .bind(query.account_id)
        .bind(query.credential_id)
        .fetch_all(&mut *self.inner)
        .await?)
    }
}
