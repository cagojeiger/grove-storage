//! Best-effort streams: callers bound latency and report failures separately.
use super::{AuditContext, Error, audit};
use grove_management_policy::Surface;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Authentication,
    Authorization,
    Reauthentication,
    Validation,
    Execution,
    Commit,
    BrowserAdmission,
}
impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::Reauthentication => "reauthentication",
            Self::Validation => "validation",
            Self::Execution => "execution",
            Self::Commit => "commit",
            Self::BrowserAdmission => "browser_admission",
        }
    }
}

#[derive(Clone, Copy)]
pub enum AuthMethod {
    Password,
    Session,
    ApiToken,
}
impl AuthMethod {
    fn name(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Session => "session",
            Self::ApiToken => "api_token",
        }
    }
}

pub struct Target {
    kind: &'static str,
    id: String,
}
impl Target {
    pub fn account(id: Uuid) -> Self {
        Self {
            kind: "account",
            id: id.to_string(),
        }
    }
    pub fn credential(id: Uuid) -> Self {
        Self {
            kind: "credential",
            id: id.to_string(),
        }
    }
    pub fn session(id: Uuid) -> Self {
        Self {
            kind: "session",
            id: id.to_string(),
        }
    }

    pub fn resource(kind: &'static str, id: &str) -> Option<Self> {
        // Lookup inputs are less restrictive than registered IDs. Do not copy
        // arbitrary lookup paths, key hashes, or credentials into history.
        let valid = matches!(kind, "storage" | "client")
            && (1..=64).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            && id.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric);
        valid.then(|| Self {
            kind,
            id: id.to_owned(),
        })
    }
}

pub struct Details {
    pub operation: &'static str,
    pub target: Option<Target>,
    pub stage: Stage,
    pub auth_method: Option<AuthMethod>,
}
impl Details {
    pub fn new(operation: &'static str, stage: Stage) -> Self {
        Self {
            operation,
            target: None,
            stage,
            auth_method: None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Outcome {
    Succeeded,
    Denied,
    Failed,
    Unknown,
}
impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy)]
pub enum SecurityReason {
    Authenticated,
    Unauthenticated,
    Forbidden,
    Unavailable,
    RateLimited,
}
impl SecurityReason {
    fn fields(self) -> (&'static str, &'static str) {
        match self {
            Self::Authenticated => ("authentication_succeeded", "authenticated"),
            Self::Unauthenticated => ("authentication_failed", "unauthenticated"),
            Self::Forbidden => ("permission_denied", "forbidden"),
            Self::Unavailable => ("authentication_unavailable", "unavailable"),
            Self::RateLimited => ("authentication_rate_limited", "rate_limited"),
        }
    }
}

pub async fn invocation(
    pool: &PgPool,
    context: &AuditContext,
    details: &Details,
    outcome: Outcome,
    error_code: Option<&'static str>,
    duration_ms: u64,
) -> Result<(), Error> {
    let f = audit::columns(Some(context.actor));
    sqlx::query("INSERT INTO management.command_invocations
        (actor_kind,actor_id,credential_id,session_id,request_id,surface,operation,outcome,error_code,duration_ms,
         target_type,target_id,stage,auth_method)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(f.kind).bind(f.actor).bind(f.credential).bind(f.session)
        .bind(context.request_id).bind(audit::surface_name(context.surface)).bind(details.operation).bind(outcome.name()).bind(error_code)
        .bind(i64::try_from(duration_ms).unwrap_or(i64::MAX))
        .bind(details.target.as_ref().map(|t| t.kind)).bind(details.target.as_ref().map(|t| &t.id))
        .bind(details.stage.name()).bind(details.auth_method.map(AuthMethod::name))
        .execute(pool).await?;
    Ok(())
}

pub async fn security(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    reason: SecurityReason,
    details: &Details,
) -> Result<(), Error> {
    let f = audit::columns(context.map(|c| c.actor));
    let (event, code) = reason.fields();
    let write = sqlx::query(
        "INSERT INTO management.security_events
        (actor_kind,actor_id,credential_id,session_id,request_id,surface,event_type,reason_code,
         operation,stage,auth_method,is_sampled)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(f.kind)
    .bind(f.actor)
    .bind(f.credential)
    .bind(f.session)
    .bind(request_id)
    .bind(audit::surface_name(surface))
    .bind(event)
    .bind(code)
    .bind(details.operation)
    .bind(details.stage.name())
    .bind(details.auth_method.map(AuthMethod::name))
    .bind(context.is_none());
    if context.is_some() {
        write.execute(pool).await?;
        return Ok(());
    }

    // Anonymous failures are sampled across replicas; no waiting on another logger.
    let mut tx = pool.begin().await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(4674380, hashtext($1))")
            .bind(format!("{}:{code}", audit::surface_name(surface)))
            .fetch_one(&mut *tx)
            .await?;
    if !locked {
        return Ok(());
    }
    let recent: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.security_events
         WHERE created_at > grove_time.wall_now() - interval '1 minute'
           AND actor_kind='anonymous' AND surface=$1 AND reason_code=$2)",
    )
    .bind(audit::surface_name(surface))
    .bind(code)
    .fetch_one(&mut *tx)
    .await?;
    if !recent {
        write.execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
