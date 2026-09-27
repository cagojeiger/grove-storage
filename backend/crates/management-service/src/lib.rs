//! Policy-enforced identity/history and resource services. Transports validate
//! their authentication envelope and select Surface. No HTTP or MCP routes here.
//! Owns configuration changes and their safety guards; object lifecycle changes
//! and physical transfers remain in the object and provider paths.
#![forbid(unsafe_code)]

mod command;
mod dispatch;
pub mod local_accounts;
mod logging;
pub mod master;
pub mod password_changes;
pub mod password_setups;
pub mod passwords;
pub mod personal_tokens;
pub mod resources;
pub mod root;
pub mod sessions;
pub use command::Command;
pub use filegate_db::management::{Proof, queries::Page};

use filegate_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, IdentityTransaction},
};
use grove_management_policy::{Surface, authorize};
use uuid::Uuid;

#[derive(Debug)]
pub enum Output {
    Identity(db::Identity),
    RootSession(db::master::Session),
    Account(Uuid),
    Changed(bool),
    Credential(db::Credential),
    Accounts(db::queries::AccountPage),
    AccountDetails(db::queries::AccountSummary),
    Credentials(Vec<db::queries::CredentialSummary>),
    Sessions(Vec<db::queries::SessionSummary>),
    Audit(Vec<db::history::AuditEvent>),
    Invocations(Vec<db::history::Invocation>),
    Security(Vec<db::history::SecurityEvent>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unauthenticated,
    Forbidden,
    NotFound,
    Conflict,
    InvalidInput,
    RequestRejected,
    Unavailable,
    OutcomeUnknown,
    RateLimited,
}
impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::InvalidInput => "invalid_input",
            Self::RequestRejected => "request_rejected",
            Self::Unavailable => "unavailable",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::RateLimited => "rate_limited",
        }
    }
}
impl From<db::Error> for Error {
    fn from(error: db::Error) -> Self {
        match error {
            db::Error::Unauthenticated => Self::Unauthenticated,
            db::Error::Forbidden => Self::Forbidden,
            db::Error::NotFound => Self::NotFound,
            db::Error::AlreadyInitialized
            | db::Error::LastAdmin
            | db::Error::InactiveAccount
            | db::Error::CredentialLimit => Self::Conflict,
            db::Error::InvalidInput => Self::InvalidInput,
            db::Error::CommitUnknown => Self::OutcomeUnknown,
            db::Error::MasterConfigurationMismatch => Self::Unavailable,
            db::Error::Database(error) => match error.as_database_error() {
                Some(error) if error.is_unique_violation() || error.is_foreign_key_violation() => {
                    Self::Conflict
                }
                Some(error) if error.is_check_violation() => Self::InvalidInput,
                _ => Self::Unavailable,
            },
        }
    }
}

pub struct Execution {
    pub request_id: Uuid,
    pub result: Result<Output, Error>,
}

pub async fn execute(
    pool: &PgPool,
    proof: Proof<'_>,
    surface: Surface,
    command: Command<'_>,
) -> Execution {
    let request_id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let operation = command.name();
    let mutation = command.is_mutation();
    let (context, result) = run(pool, proof, surface, command, request_id).await;
    let result = if !mutation && matches!(result, Err(Error::OutcomeUnknown)) {
        Err(Error::Unavailable)
    } else {
        result
    };
    logging::record(
        pool,
        context.as_ref(),
        request_id,
        surface,
        operation,
        started.elapsed(),
        &result,
    )
    .await;
    Execution { request_id, result }
}

async fn run(
    pool: &PgPool,
    proof: Proof<'_>,
    surface: Surface,
    command: Command<'_>,
    request_id: Uuid,
) -> (Option<AuditContext>, Result<Output, Error>) {
    let mut tx = match IdentityTransaction::begin(pool).await {
        Ok(tx) => tx,
        Err(error) => return (None, Err(error.into())),
    };
    let identity = match tx.resolve(proof).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return (None, Err(Error::Unauthenticated)),
        Err(error) => return (None, Err(error.into())),
    };
    let context = match audit_context(&identity, request_id, surface) {
        Ok(context) => context,
        Err(error) => return (None, Err(error)),
    };
    let scope = match authorize(identity.caller(), surface, command.action()) {
        Ok(scope) => scope,
        Err(_) => return (Some(context), Err(Error::Forbidden)),
    };
    let result = dispatch::run(tx, &context, identity, scope, command)
        .await
        .map_err(Error::from);
    (Some(context), result)
}

fn audit_context(
    identity: &db::ResolvedIdentity,
    request_id: Uuid,
    surface: Surface,
) -> Result<AuditContext, Error> {
    let actor = match identity {
        db::ResolvedIdentity::User(identity) => AuditActor::User {
            id: identity.account_id,
            credential_id: identity.credential_id,
            session_id: identity.session_id,
        },
        db::ResolvedIdentity::Root(session) => AuditActor::Master {
            session_id: Some(session.id),
        },
    };
    Ok(AuditContext {
        actor,
        request_id,
        surface,
    })
}
