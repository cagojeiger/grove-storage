use super::{Error, IdentityTransaction, queries::Page};
use chrono::{DateTime, Utc};
use uuid::Uuid;

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
        page: Page<i64>,
    ) -> Result<Vec<AuditEvent>, Error> {
        Ok(sqlx::query_as("SELECT * FROM management.audit_events WHERE ($1::uuid IS NULL OR actor_id=$1 OR owner_user_id=$1)
            AND ($2::bigint IS NULL OR id<$2) ORDER BY id DESC LIMIT $3")
            .bind(scope.user()).bind(page.before).bind(page.limit).fetch_all(&mut *self.inner).await?)
    }
    pub async fn invocation_history(
        &mut self,
        scope: HistoryScope,
        page: Page<i64>,
    ) -> Result<Vec<Invocation>, Error> {
        Ok(sqlx::query_as("SELECT * FROM management.command_invocations WHERE ($1::uuid IS NULL OR actor_id=$1 OR owner_user_id=$1)
            AND ($2::bigint IS NULL OR id<$2) ORDER BY id DESC LIMIT $3")
            .bind(scope.user()).bind(page.before).bind(page.limit).fetch_all(&mut *self.inner).await?)
    }
    pub async fn security_history(&mut self, page: Page<i64>) -> Result<Vec<SecurityEvent>, Error> {
        Ok(sqlx::query_as("SELECT * FROM management.security_events WHERE ($1::bigint IS NULL OR id<$1) ORDER BY id DESC LIMIT $2")
            .bind(page.before).bind(page.limit).fetch_all(&mut *self.inner).await?)
    }
}
