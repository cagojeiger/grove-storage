//! Management identity persistence, not an HTTP authentication boundary.
//!
//! Callers must authenticate, authorize, and validate CSRF before mutations.
//! AuditContext is trusted server context, never client input. These primitives
//! serialize identity mutations and commit their audit records atomically.

mod accounts;
mod audit;
mod credentials;
pub mod history;
mod identity;
pub mod master;
pub mod queries;
mod resource_writes;
mod resources;
mod root;
mod sessions;
mod storage_writes;
pub mod telemetry;
mod transaction;

pub use accounts::{AccountChange, NewAccount, bootstrap, change_account, create_account};
pub use audit::{AuditActor, AuditContext};
pub use credentials::{
    Credential, NewCredential, issue_credential, recover_admin, revoke_credential,
};
pub use identity::{Identity, authenticate, session_actor};
pub use resource_writes::EncryptedServiceCredential;
pub use sessions::{Session, create_session, revoke_session};
pub use transaction::{IdentityTransaction, Proof, ResolvedIdentity};

use sqlx::{PgPool, Postgres, Transaction};

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    AlreadyInitialized,
    NotFound,
    InactiveAccount,
    LastAdmin,
    InvalidInput,
    CommitUnknown,
    MasterConfigurationMismatch,
    CredentialLimit,
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

// Small management population: one lock orders bootstrap, account changes,
// issuance, recovery, login, and revocation. Data-plane work never takes it.
async fn lock(pool: &PgPool) -> Result<Transaction<'_, Postgres>, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

fn role_name(role: grove_management_policy::Role) -> &'static str {
    use grove_management_policy::Role;
    match role {
        Role::Reader => "reader",
        Role::Writer => "writer",
        Role::Admin => "admin",
    }
}
